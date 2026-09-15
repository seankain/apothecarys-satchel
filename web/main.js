// Wires the seed box to the wasm module and the module's payload to the
// renderer.
//
// The module exports a plain C ABI — no wasm-bindgen, no generated glue — so
// the only contract between this file and `crates/web-demo/src/lib.rs` is the
// byte layout decoded in `decodeMesh` below. `crates/web-demo/tests/payload.rs`
// decodes the same bytes natively; if the two ever disagree, that test is
// where it shows up.

import { PlantRenderer } from './renderer.js';

const WASM_URL = './plant.wasm';
/// "PLNT", little-endian, matching `apothecarys_web_demo::MAGIC`.
const MAGIC = 0x544e4c50;
/// Matching `apothecarys_web_demo::VERSION`.
const VERSION = 1;
/// Bytes before the first batch header.
const PRELUDE_BYTES = 16;
/// Bytes per batch header.
const BATCH_HEADER_BYTES = 36;
/// `u64::MAX`, the largest seed `StdRng::seed_from_u64` takes.
const MAX_SEED = (1n << 64n) - 1n;

const elements = {
  canvas: document.getElementById('scene'),
  seed: document.getElementById('seed'),
  regenerate: document.getElementById('regenerate'),
  randomize: document.getElementById('randomize'),
  status: document.getElementById('status'),
  stats: document.getElementById('stats'),
  traits: document.getElementById('traits'),
  effects: document.getElementById('effects'),
};

let wasm = null;
let renderer = null;

/** Reports a problem in the status line and on the console. */
function fail(message, error) {
  elements.status.textContent = message;
  elements.status.dataset.state = 'error';
  if (error) {
    console.error(error);
  }
}

/**
 * Instantiates the module.
 *
 * `instantiateStreaming` needs the server to send `application/wasm`; GitLab
 * Pages does, but a plain `python3 -m http.server` on an older Python may not,
 * so the `arrayBuffer` path is kept as the fallback rather than left to chance.
 */
async function loadWasm() {
  const response = await fetch(WASM_URL);
  if (!response.ok) {
    throw new Error(`${WASM_URL} returned ${response.status}`);
  }
  try {
    const { instance } = await WebAssembly.instantiateStreaming(response.clone(), {});
    return instance.exports;
  } catch {
    const { instance } = await WebAssembly.instantiate(await response.arrayBuffer(), {});
    return instance.exports;
  }
}

/**
 * Decodes the mesh payload `mesh_payload` wrote.
 *
 * The payload is copied out of the module's memory whole, once, before
 * anything is read from it. That is not only about detachment — the next
 * `plant_generate` can grow the wasm heap, which detaches every view onto the
 * old buffer — but about alignment: the payload is a `Vec<u8>`, so its address
 * carries no alignment guarantee, and `new Float32Array(buffer, offset, …)`
 * throws unless `offset` is a multiple of four. Slicing first gives a fresh
 * `ArrayBuffer` starting at zero, where every section offset is aligned by
 * construction, and leaves the typed arrays safe to hand straight to
 * `bufferData`.
 */
function decodeMesh(memory, pointer, length) {
  const payload = new Uint8Array(memory.buffer, pointer, length).slice().buffer;
  const view = new DataView(payload);

  const magic = view.getUint32(0, true);
  if (magic !== MAGIC) {
    throw new Error(`plant.wasm returned an unrecognised payload (0x${magic.toString(16)})`);
  }
  const version = view.getUint32(4, true);
  if (version !== VERSION) {
    throw new Error(`plant.wasm speaks payload version ${version}, this page speaks ${VERSION}`);
  }

  const batchCount = view.getUint32(8, true);
  const batches = [];
  for (let index = 0; index < batchCount; index += 1) {
    const header = PRELUDE_BYTES + BATCH_HEADER_BYTES * index;
    const vertexCount = view.getUint32(header, true);
    const indexCount = view.getUint32(header + 4, true);
    const positionsOffset = view.getUint32(header + 8, true);
    const normalsOffset = view.getUint32(header + 12, true);
    const indicesOffset = view.getUint32(header + 16, true);

    batches.push({
      positions: new Float32Array(payload, positionsOffset, vertexCount * 3),
      normals: new Float32Array(payload, normalsOffset, vertexCount * 3),
      indices: new Uint32Array(payload, indicesOffset, indexCount),
      color: new Float32Array([
        view.getFloat32(header + 20, true),
        view.getFloat32(header + 24, true),
        view.getFloat32(header + 28, true),
        view.getFloat32(header + 32, true),
      ]),
    });
  }
  return batches;
}

/** The module's UTF-8 JSON metadata for the plant it last grew. */
function readInfo() {
  const bytes = new Uint8Array(
    wasm.memory.buffer,
    wasm.plant_info_ptr(),
    wasm.plant_info_len(),
  );
  return JSON.parse(new TextDecoder().decode(bytes));
}

/** Grows the plant for `seed`, draws it, and fills the panel beside it. */
function generate(seed) {
  const started = performance.now();
  const status = wasm.plant_generate(
    Number(seed & 0xffffffffn),
    Number(seed >> 32n),
    0, // LodTier::Hub — the quality a plant is drawn at beside the player.
  );
  const info = readInfo();
  if (status !== 0) {
    fail(`Seed ${seed} could not be grown: ${info.error ?? 'unknown error'}`);
    return;
  }

  const batches = decodeMesh(wasm.memory, wasm.plant_mesh_ptr(), wasm.plant_mesh_len());
  const elapsed = performance.now() - started;
  renderer.setPlant(batches);
  describe(info, elapsed);

  elements.status.dataset.state = 'ok';
  elements.status.textContent =
    `Grown in ${elapsed.toFixed(1)} ms — ${info.triangle_count.toLocaleString()} triangles` +
    ` in ${info.batch_count} draw call${info.batch_count === 1 ? '' : 's'}.`;
}

/** Fills the three lists beside the canvas. */
function describe(info, elapsed) {
  const number = (value, digits = 2) =>
    value.toLocaleString(undefined, {
      minimumFractionDigits: digits,
      maximumFractionDigits: digits,
    });

  elements.stats.replaceChildren(
    ...rows([
      ['Seed', info.seed.toString()],
      ['Quality tier', info.lod],
      ['Generated in', `${number(elapsed, 1)} ms`],
      ['Triangles', `${info.triangle_count.toLocaleString()} / ${info.triangle_budget.toLocaleString()}`],
      ['Vertices', info.vertex_count.toLocaleString()],
      ['Draw calls', info.batch_count.toString()],
      ['Height', `${number(info.height)} u`],
      ['Surface area', `${number(info.surface_area)} u²`],
      ['Volume', `${number(info.volume, 3)} u³`],
    ]),
  );

  elements.traits.replaceChildren(
    ...rows([
      ['L-system iterations', info.iterations.toString()],
      ['Derived symbols', info.symbol_count.toLocaleString()],
      ['Stem segments', info.segment_count.toLocaleString()],
      ['Branch angle', `${number(info.branch_angle, 1)}°`],
      ['Branch length', number(info.branch_length)],
      ['Branch thickness', number(info.branch_thickness, 3)],
      ['Branching factor', info.branching_factor.toString()],
      ['Cross-section', `profile ${info.cross_section_index}`],
      ['Taper', info.taper_curve],
      ['Tropism elasticity', number(info.tropism_elasticity, 3)],
      ['Axis curvature', `${number(info.axis_curvature, 1)}°/segment`],
      ['Leaves', `${info.leaf_count.toLocaleString()} — template ${info.leaf_mesh_index}, ${number(info.leaf_scale)}×`, info.leaf_color],
      [
        'Petals',
        info.produces_flowers ? `${info.petal_count.toLocaleString()}` : 'none',
        info.produces_flowers ? info.petal_color : null,
      ],
      [
        'Fruit',
        info.produces_fruit ? `${info.fruit_count.toLocaleString()}` : 'none',
        info.produces_fruit ? info.fruit_color : null,
      ],
    ]),
  );

  elements.effects.replaceChildren(
    ...info.alchemy_effects.map((effect) => {
      const item = document.createElement('li');
      item.textContent = effect;
      return item;
    }),
  );
}

/** `[label, value, swatch?]` triples as definition-list rows. */
function rows(entries) {
  return entries.flatMap(([label, value, swatch]) => {
    const term = document.createElement('dt');
    term.textContent = label;
    const definition = document.createElement('dd');
    if (swatch) {
      const chip = document.createElement('span');
      chip.className = 'swatch';
      chip.style.background = swatch;
      chip.title = swatch;
      definition.append(chip);
    }
    definition.append(value);
    return [term, definition];
  });
}

/**
 * The seed in the box, or `null` if it is not one.
 *
 * `BigInt` rather than `Number`: the module takes a `u64`, and a `Number`
 * silently loses the low bits of anything past 2^53, which would make two
 * visibly different seeds grow the same plant.
 */
function seedFromInput() {
  const raw = elements.seed.value.trim();
  if (!/^\d+$/.test(raw)) {
    return null;
  }
  const value = BigInt(raw);
  return value > MAX_SEED ? null : value;
}

/** A fresh seed from the browser's CSPRNG, uniform over the whole `u64` range. */
function randomSeed() {
  const bytes = new BigUint64Array(1);
  crypto.getRandomValues(bytes);
  return bytes[0];
}

/** Reads, validates and applies whatever is in the box. */
function regenerate() {
  const seed = seedFromInput();
  if (seed === null) {
    fail(`A seed is a whole number from 0 to ${MAX_SEED}.`);
    elements.seed.focus();
    elements.seed.select();
    return;
  }
  // So a plant can be linked to, and the back button walks the seeds tried.
  const url = new URL(window.location.href);
  if (url.searchParams.get('seed') !== seed.toString()) {
    url.searchParams.set('seed', seed.toString());
    window.history.pushState({ seed: seed.toString() }, '', url);
  }
  generate(seed);
}

async function main() {
  try {
    renderer = new PlantRenderer(elements.canvas);
  } catch (error) {
    fail(error.message, error);
    return;
  }

  try {
    wasm = await loadWasm();
  } catch (error) {
    fail(`Could not load ${WASM_URL}. The page has to be served over HTTP, not opened as a file.`, error);
    return;
  }

  if (wasm.plant_payload_version() !== VERSION) {
    fail(
      `plant.wasm speaks payload version ${wasm.plant_payload_version()}, this page speaks ${VERSION}.` +
        ' A hard refresh should clear the cached copy.',
    );
    return;
  }

  elements.regenerate.addEventListener('click', regenerate);
  elements.randomize.addEventListener('click', () => {
    elements.seed.value = randomSeed().toString();
    regenerate();
  });
  elements.seed.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') {
      regenerate();
    }
  });
  window.addEventListener('popstate', () => {
    elements.seed.value = new URLSearchParams(window.location.search).get('seed') ?? '42';
    regenerate();
  });

  const requested = new URLSearchParams(window.location.search).get('seed');
  elements.seed.value = requested && /^\d+$/.test(requested) ? requested : '42';
  elements.regenerate.disabled = false;
  elements.randomize.disabled = false;
  elements.seed.disabled = false;
  generate(seedFromInput() ?? 42n);
}

main();
