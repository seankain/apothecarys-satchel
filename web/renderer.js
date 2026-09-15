// A WebGL2 view of one plant: orbit camera, two lights, a faded ground disc.
//
// Deliberately dependency-free. The page is served from GitLab Pages with no
// bundler and no CDN, so everything it needs — the matrix maths, the shaders,
// the input handling — is here, and the whole deploy is four static files plus
// a `.wasm`.
//
// Vertices arrive in world space and stay there: the model matrix is the
// identity, so `aNormal` is already a world normal and the fragment shader can
// light it without a normal matrix.

const VERTEX_SHADER = `#version 300 es
in vec3 aPosition;
in vec3 aNormal;

uniform mat4 uViewProjection;

out vec3 vNormal;
out vec3 vWorldPosition;

void main() {
  vNormal = aNormal;
  vWorldPosition = aPosition;
  gl_Position = uViewProjection * vec4(aPosition, 1.0);
}
`;

const FRAGMENT_SHADER = `#version 300 es
precision highp float;

in vec3 vNormal;
in vec3 vWorldPosition;

uniform vec4 uColor;
uniform vec3 uCameraPosition;

out vec4 outColor;

const vec3 KEY_DIRECTION = normalize(vec3(0.45, 0.80, 0.40));
const vec3 KEY_COLOR = vec3(1.00, 0.97, 0.90);
const vec3 FILL_DIRECTION = normalize(vec3(-0.55, 0.20, -0.45));
const vec3 FILL_COLOR = vec3(0.35, 0.45, 0.60);
// The ambient floor is deliberately high. A turtle's organs are flat patches,
// so a leaf turned away from both lights has nothing to catch and reads as a
// hole in the plant rather than as a leaf in shadow.
const vec3 SKY_AMBIENT = vec3(0.30, 0.35, 0.40);
const vec3 GROUND_AMBIENT = vec3(0.20, 0.19, 0.16);

void main() {
  // A turtle draws open tubes and single-sided organ patches, so roughly half
  // of what the camera sees is a back face. Flipping the normal toward the
  // viewer is what stops leaves reading as black holes.
  vec3 normal = normalize(vNormal);
  if (!gl_FrontFacing) {
    normal = -normal;
  }

  vec3 ambient = mix(GROUND_AMBIENT, SKY_AMBIENT, 0.5 + 0.5 * normal.y);
  float key = max(dot(normal, KEY_DIRECTION), 0.0);
  float fill = max(dot(normal, FILL_DIRECTION), 0.0);

  vec3 viewDirection = normalize(uCameraPosition - vWorldPosition);
  vec3 halfway = normalize(viewDirection + KEY_DIRECTION);
  float specular = pow(max(dot(normal, halfway), 0.0), 28.0) * 0.20;

  vec3 lit = uColor.rgb * (ambient + KEY_COLOR * key * 0.85 + FILL_COLOR * fill * 0.45)
           + KEY_COLOR * specular;
  outColor = vec4(min(lit, vec3(1.0)), uColor.a);
}
`;

const GROUND_VERTEX_SHADER = `#version 300 es
in vec2 aCorner;

uniform mat4 uViewProjection;
uniform float uRadius;
uniform vec3 uCenter;

out vec2 vPlane;

void main() {
  vPlane = aCorner * uRadius;
  gl_Position = uViewProjection * vec4(vPlane.x + uCenter.x, 0.0, vPlane.y + uCenter.z, 1.0);
}
`;

const GROUND_FRAGMENT_SHADER = `#version 300 es
precision highp float;

in vec2 vPlane;

uniform float uRadius;
uniform float uGridStep;

out vec4 outColor;

const vec3 GROUND_COLOR = vec3(0.13, 0.15, 0.14);
const vec3 GRID_COLOR = vec3(0.34, 0.40, 0.36);

void main() {
  float radius = length(vPlane) / uRadius;
  if (radius > 1.0) {
    discard;
  }

  // One screen-space-wide line per grid cell: dividing the distance to the
  // nearest cell edge by its own derivative keeps the line a constant width
  // however far away the floor is, instead of aliasing into noise.
  vec2 cell = vPlane / uGridStep;
  vec2 distanceToEdge = abs(fract(cell) - 0.5) / fwidth(cell);
  float line = 1.0 - min(min(distanceToEdge.x, distanceToEdge.y), 1.0);

  float fade = 1.0 - smoothstep(0.25, 1.0, radius);
  outColor = vec4(mix(GROUND_COLOR, GRID_COLOR, line * 0.7), fade * (0.55 + line * 0.35));
}
`;

// --- Matrices, column-major, the order WebGL uniforms expect ---------------

function perspective(fovY, aspect, near, far) {
  const f = 1 / Math.tan(fovY / 2);
  const range = 1 / (near - far);
  return new Float32Array([
    f / aspect, 0, 0, 0,
    0, f, 0, 0,
    0, 0, (near + far) * range, -1,
    0, 0, near * far * range * 2, 0,
  ]);
}

function lookAt(eye, target, up) {
  const z = normalize(subtract(eye, target));
  const x = normalize(cross(up, z));
  const y = cross(z, x);
  return new Float32Array([
    x[0], y[0], z[0], 0,
    x[1], y[1], z[1], 0,
    x[2], y[2], z[2], 0,
    -dot(x, eye), -dot(y, eye), -dot(z, eye), 1,
  ]);
}

function multiply(a, b) {
  const out = new Float32Array(16);
  for (let column = 0; column < 4; column += 1) {
    for (let row = 0; row < 4; row += 1) {
      let sum = 0;
      for (let k = 0; k < 4; k += 1) {
        sum += a[k * 4 + row] * b[column * 4 + k];
      }
      out[column * 4 + row] = sum;
    }
  }
  return out;
}

const subtract = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a, b) => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];

function normalize(v) {
  const length = Math.hypot(v[0], v[1], v[2]) || 1;
  return [v[0] / length, v[1] / length, v[2] / length];
}

// --- Shader plumbing -------------------------------------------------------

function compile(gl, type, source) {
  const shader = gl.createShader(type);
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const log = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error(`shader failed to compile: ${log}`);
  }
  return shader;
}

function link(gl, vertexSource, fragmentSource) {
  const program = gl.createProgram();
  gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, vertexSource));
  gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, fragmentSource));
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(program);
    gl.deleteProgram(program);
    throw new Error(`program failed to link: ${log}`);
  }
  return program;
}

const FIELD_OF_VIEW = (45 * Math.PI) / 180;
/// Degrees of yaw a second while the camera is left alone. Slow enough to
/// read as a turntable rather than as motion the page is imposing.
const IDLE_SPIN_DEGREES_PER_SECOND = 9;
/// How close to straight up or down the camera may be pitched. Reaching either
/// pole would make the up vector and the view direction parallel, and
/// `lookAt`'s cross product degenerate.
const PITCH_LIMIT = Math.PI / 2 - 0.05;

export class PlantRenderer {
  /**
   * @param {HTMLCanvasElement} canvas
   */
  constructor(canvas) {
    const gl = canvas.getContext('webgl2', { antialias: true, alpha: false });
    if (!gl) {
      throw new Error('WebGL2 is unavailable in this browser.');
    }

    this.canvas = canvas;
    this.gl = gl;
    this.program = link(gl, VERTEX_SHADER, FRAGMENT_SHADER);
    this.groundProgram = link(gl, GROUND_VERTEX_SHADER, GROUND_FRAGMENT_SHADER);

    this.uniforms = {
      viewProjection: gl.getUniformLocation(this.program, 'uViewProjection'),
      color: gl.getUniformLocation(this.program, 'uColor'),
      cameraPosition: gl.getUniformLocation(this.program, 'uCameraPosition'),
    };
    this.groundUniforms = {
      viewProjection: gl.getUniformLocation(this.groundProgram, 'uViewProjection'),
      radius: gl.getUniformLocation(this.groundProgram, 'uRadius'),
      center: gl.getUniformLocation(this.groundProgram, 'uCenter'),
      gridStep: gl.getUniformLocation(this.groundProgram, 'uGridStep'),
    };

    this.ground = this.createGround();
    this.batches = [];
    this.bounds = null;

    this.camera = { yaw: 0.6, pitch: 0.28, distance: 4, target: [0, 1, 0] };
    this.spinning = true;
    this.lastFrame = 0;

    this.bindInput();
    this.resize();
    window.addEventListener('resize', () => this.resize());
    requestAnimationFrame((now) => this.frame(now));
  }

  /** A unit quad, stretched into the ground disc by the vertex shader. */
  createGround() {
    const gl = this.gl;
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(
      gl.ARRAY_BUFFER,
      new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]),
      gl.STATIC_DRAW,
    );
    const corner = gl.getAttribLocation(this.groundProgram, 'aCorner');
    gl.enableVertexAttribArray(corner);
    gl.vertexAttribPointer(corner, 2, gl.FLOAT, false, 0, 0);
    gl.bindVertexArray(null);
    return { vao, count: 4 };
  }

  /**
   * Replaces what is drawn, and frames the camera on it.
   *
   * @param {{positions: Float32Array, normals: Float32Array, indices: Uint32Array, color: number[]}[]} batches
   */
  setPlant(batches) {
    const gl = this.gl;
    for (const batch of this.batches) {
      gl.deleteVertexArray(batch.vao);
      for (const buffer of batch.buffers) {
        gl.deleteBuffer(buffer);
      }
    }

    const position = gl.getAttribLocation(this.program, 'aPosition');
    const normal = gl.getAttribLocation(this.program, 'aNormal');

    this.batches = batches.map((batch) => {
      const vao = gl.createVertexArray();
      gl.bindVertexArray(vao);

      const positions = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, positions);
      gl.bufferData(gl.ARRAY_BUFFER, batch.positions, gl.STATIC_DRAW);
      gl.enableVertexAttribArray(position);
      gl.vertexAttribPointer(position, 3, gl.FLOAT, false, 0, 0);

      const normals = gl.createBuffer();
      gl.bindBuffer(gl.ARRAY_BUFFER, normals);
      gl.bufferData(gl.ARRAY_BUFFER, batch.normals, gl.STATIC_DRAW);
      gl.enableVertexAttribArray(normal);
      gl.vertexAttribPointer(normal, 3, gl.FLOAT, false, 0, 0);

      const indices = gl.createBuffer();
      gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indices);
      gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, batch.indices, gl.STATIC_DRAW);

      gl.bindVertexArray(null);
      return {
        vao,
        buffers: [positions, normals, indices],
        count: batch.indices.length,
        color: batch.color,
      };
    });

    // Opaque first, so what is behind a translucent petal has already been
    // drawn and depth-tested by the time the petal blends over it.
    this.batches.sort((a, b) => b.color[3] - a.color[3]);

    this.bounds = bounds(batches);
    this.frameCamera();
  }

  /** Points the camera at the plant and backs off far enough to see all of it. */
  frameCamera() {
    if (!this.bounds) {
      return;
    }
    const { min, max } = this.bounds;
    const centre = [(min[0] + max[0]) / 2, (min[1] + max[1]) / 2, (min[2] + max[2]) / 2];
    const radius = Math.max(
      0.5 * Math.hypot(max[0] - min[0], max[1] - min[1], max[2] - min[2]),
      0.05,
    );
    this.camera.target = centre;
    // A little further than the bounding sphere strictly needs, so a plant
    // does not sit flush against the edges of the canvas.
    this.camera.distance = (radius / Math.sin(FIELD_OF_VIEW / 2)) * 1.25;
    this.groundRadius = Math.max(radius * 3.0, 1.0);
    this.gridStep = niceStep(this.groundRadius / 6);
  }

  bindInput() {
    const canvas = this.canvas;
    let dragging = null;

    const start = (x, y, id) => {
      dragging = { x, y, id };
      this.spinning = false;
      canvas.classList.add('dragging');
    };
    const move = (x, y) => {
      if (!dragging) {
        return;
      }
      this.camera.yaw -= (x - dragging.x) * 0.0075;
      this.camera.pitch = Math.max(
        -PITCH_LIMIT,
        Math.min(PITCH_LIMIT, this.camera.pitch + (y - dragging.y) * 0.0075),
      );
      dragging.x = x;
      dragging.y = y;
    };
    const end = () => {
      dragging = null;
      canvas.classList.remove('dragging');
    };

    canvas.addEventListener('pointerdown', (event) => {
      canvas.setPointerCapture(event.pointerId);
      start(event.clientX, event.clientY, event.pointerId);
    });
    canvas.addEventListener('pointermove', (event) => move(event.clientX, event.clientY));
    canvas.addEventListener('pointerup', end);
    canvas.addEventListener('pointercancel', end);

    canvas.addEventListener(
      'wheel',
      (event) => {
        event.preventDefault();
        this.spinning = false;
        this.camera.distance = Math.max(
          0.2,
          Math.min(200, this.camera.distance * Math.exp(event.deltaY * 0.0012)),
        );
      },
      { passive: false },
    );
  }

  resize() {
    const ratio = Math.min(window.devicePixelRatio || 1, 2);
    const width = Math.max(1, Math.round(this.canvas.clientWidth * ratio));
    const height = Math.max(1, Math.round(this.canvas.clientHeight * ratio));
    if (this.canvas.width !== width || this.canvas.height !== height) {
      this.canvas.width = width;
      this.canvas.height = height;
    }
  }

  frame(now) {
    const elapsed = this.lastFrame ? (now - this.lastFrame) / 1000 : 0;
    this.lastFrame = now;
    if (this.spinning) {
      this.camera.yaw += (IDLE_SPIN_DEGREES_PER_SECOND * Math.PI) / 180 * elapsed;
    }
    this.resize();
    this.draw();
    requestAnimationFrame((next) => this.frame(next));
  }

  draw() {
    const gl = this.gl;
    const { width, height } = this.canvas;

    gl.viewport(0, 0, width, height);
    gl.clearColor(0.055, 0.063, 0.071, 1);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.enable(gl.DEPTH_TEST);
    // Culling stays off: organs are single-sided patches and stems are open
    // tubes, so their back faces are part of the silhouette, not hidden by it.
    gl.disable(gl.CULL_FACE);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

    const { yaw, pitch, distance, target } = this.camera;
    const eye = [
      target[0] + distance * Math.cos(pitch) * Math.sin(yaw),
      target[1] + distance * Math.sin(pitch),
      target[2] + distance * Math.cos(pitch) * Math.cos(yaw),
    ];
    const viewProjection = multiply(
      perspective(FIELD_OF_VIEW, width / height, Math.max(distance * 0.002, 0.005), distance * 12),
      lookAt(eye, target, [0, 1, 0]),
    );

    if (this.bounds) {
      gl.useProgram(this.groundProgram);
      gl.uniformMatrix4fv(this.groundUniforms.viewProjection, false, viewProjection);
      gl.uniform1f(this.groundUniforms.radius, this.groundRadius);
      gl.uniform1f(this.groundUniforms.gridStep, this.gridStep);
      gl.uniform3fv(this.groundUniforms.center, new Float32Array(target));
      gl.bindVertexArray(this.ground.vao);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, this.ground.count);
    }

    gl.useProgram(this.program);
    gl.uniformMatrix4fv(this.uniforms.viewProjection, false, viewProjection);
    gl.uniform3fv(this.uniforms.cameraPosition, new Float32Array(eye));
    for (const batch of this.batches) {
      gl.uniform4fv(this.uniforms.color, batch.color);
      gl.bindVertexArray(batch.vao);
      gl.drawElements(gl.TRIANGLES, batch.count, gl.UNSIGNED_INT, 0);
    }
    gl.bindVertexArray(null);
  }
}

/** The axis-aligned box every batch's vertices fit inside. */
function bounds(batches) {
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  for (const batch of batches) {
    const { positions } = batch;
    for (let i = 0; i < positions.length; i += 3) {
      for (let axis = 0; axis < 3; axis += 1) {
        const value = positions[i + axis];
        if (value < min[axis]) min[axis] = value;
        if (value > max[axis]) max[axis] = value;
      }
    }
  }
  return Number.isFinite(min[0]) ? { min, max } : null;
}

/** The nearest 1, 2 or 5 times a power of ten — a grid spacing that reads. */
function niceStep(rough) {
  const magnitude = 10 ** Math.floor(Math.log10(Math.max(rough, 1e-6)));
  const normalized = rough / magnitude;
  const step = normalized < 1.5 ? 1 : normalized < 3.5 ? 2 : normalized < 7.5 ? 5 : 10;
  return step * magnitude;
}
