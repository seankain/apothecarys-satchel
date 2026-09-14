"""Flat-shaded orthographic render of a Wavefront OBJ, stdlib only.

Used to produce the before/after comparison for issue #21: the pre-port and
post-port geometry for the same seed, from the same camera, at the same scale.
Not a game screenshot -- it is the exported geometry, which is the thing that
actually changed.
"""
import math, struct, sys, zlib

def parse_mtl(path):
    materials, current = {}, None
    try:
        text = open(path).read()
    except OSError:
        return materials
    for line in text.splitlines():
        parts = line.split()
        if not parts:
            continue
        if parts[0] == "newmtl":
            current = parts[1]
            materials[current] = (0.6, 0.6, 0.6)
        elif parts[0] == "Kd" and current:
            materials[current] = tuple(float(v) for v in parts[1:4])
    return materials

def parse_obj(path):
    verts, tris = [], []
    material = None
    for line in open(path):
        parts = line.split()
        if not parts:
            continue
        if parts[0] == "v":
            verts.append(tuple(float(v) for v in parts[1:4]))
        elif parts[0] == "usemtl":
            material = parts[1]
        elif parts[0] == "f":
            idx = [int(p.split("/")[0]) for p in parts[1:]]
            idx = [i - 1 if i > 0 else len(verts) + i for i in idx]
            for k in range(1, len(idx) - 1):
                tris.append((idx[0], idx[k], idx[k + 1], material))
    return verts, tris

def bbox(verts):
    lo = [min(v[i] for v in verts) for i in range(3)]
    hi = [max(v[i] for v in verts) for i in range(3)]
    return lo, hi

def render(models, size, out, bg=(250, 249, 245)):
    """models: list of (verts, tris, materials). One panel each, same camera."""
    # One camera for every panel: the union of their boxes, so the size
    # difference between before and after is visible rather than normalised away.
    allv = [v for verts, _, _ in models for v in verts]
    lo, hi = bbox(allv)
    centre = [(lo[i] + hi[i]) / 2 for i in range(3)]
    extent = max(hi[i] - lo[i] for i in range(3)) or 1.0

    # Orthographic three-quarter view, +Y up.
    eye = (0.72, 0.45, 0.72)
    n = math.sqrt(sum(c * c for c in eye))
    fwd = tuple(-c / n for c in eye)
    up0 = (0.0, 1.0, 0.0)
    right = (fwd[1] * up0[2] - fwd[2] * up0[1],
             fwd[2] * up0[0] - fwd[0] * up0[2],
             fwd[0] * up0[1] - fwd[1] * up0[0])
    rn = math.sqrt(sum(c * c for c in right))
    right = tuple(c / rn for c in right)
    up = (right[1] * fwd[2] - right[2] * fwd[1],
          right[2] * fwd[0] - right[0] * fwd[2],
          right[0] * fwd[1] - right[1] * fwd[0])
    light = (0.45, 0.78, 0.44)

    panel_w, h = size
    width = panel_w * len(models)
    pix = bytearray()
    for _ in range(width * h):
        pix += bytes(bg)

    scale = min(panel_w, h) / (extent * 1.15)

    for panel, (verts, tris, materials) in enumerate(models):
        x0 = panel * panel_w
        depth = [None] * (panel_w * h)
        shaded = []
        for a, b, c, mat in tris:
            try:
                pa, pb, pc = verts[a], verts[b], verts[c]
            except IndexError:
                continue
            u = tuple(pb[i] - pa[i] for i in range(3))
            v = tuple(pc[i] - pa[i] for i in range(3))
            nx = u[1] * v[2] - u[2] * v[1]
            ny = u[2] * v[0] - u[0] * v[2]
            nz = u[0] * v[1] - u[1] * v[0]
            nl = math.sqrt(nx * nx + ny * ny + nz * nz)
            if nl < 1e-12:
                continue
            nrm = (nx / nl, ny / nl, nz / nl)
            lam = abs(sum(nrm[i] * light[i] for i in range(3)))
            base = materials.get(mat, (0.55, 0.55, 0.55))
            col = tuple(min(255, int(255 * (0.28 + 0.72 * lam) * ch)) for ch in base)

            screen = []
            for p in (pa, pb, pc):
                d = tuple(p[i] - centre[i] for i in range(3))
                sx = sum(d[i] * right[i] for i in range(3)) * scale + panel_w / 2
                sy = -sum(d[i] * up[i] for i in range(3)) * scale + h / 2
                sz = sum(d[i] * fwd[i] for i in range(3))
                screen.append((sx, sy, sz))
            shaded.append((screen, col))

        for screen, col in shaded:
            (ax, ay, az), (bx, by, bz), (cx, cy, cz) = screen
            minx, maxx = max(0, int(min(ax, bx, cx))), min(panel_w - 1, int(max(ax, bx, cx)) + 1)
            miny, maxy = max(0, int(min(ay, by, cy))), min(h - 1, int(max(ay, by, cy)) + 1)
            area = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
            if abs(area) < 1e-9:
                continue
            for py in range(miny, maxy + 1):
                for px in range(minx, maxx + 1):
                    fx, fy = px + 0.5, py + 0.5
                    w0 = ((bx - ax) * (fy - ay) - (by - ay) * (fx - ax)) / area
                    w1 = ((fx - ax) * (cy - ay) - (fy - ay) * (cx - ax)) / area
                    if w0 < 0 or w1 < 0 or w0 + w1 > 1:
                        continue
                    z = az + w1 * (bz - az) + w0 * (cz - az)
                    slot = py * panel_w + px
                    if depth[slot] is not None and z <= depth[slot]:
                        continue
                    depth[slot] = z
                    off = ((py * width) + x0 + px) * 3
                    pix[off:off + 3] = bytes(col)

    raw = b"".join(b"\x00" + bytes(pix[y * width * 3:(y + 1) * width * 3]) for y in range(h))
    def chunk(tag, data):
        return (struct.pack(">I", len(data)) + tag + data
                + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))
    png = (b"\x89PNG\r\n\x1a\n"
           + chunk(b"IHDR", struct.pack(">IIBBBBB", width, h, 8, 2, 0, 0, 0))
           + chunk(b"IDAT", zlib.compress(raw, 9))
           + chunk(b"IEND", b""))
    open(out, "wb").write(png)

if __name__ == "__main__":
    out = sys.argv[1]
    models = []
    for obj in sys.argv[2:]:
        verts, tris = parse_obj(obj)
        models.append((verts, tris, parse_mtl(obj[:-4] + ".mtl")))
    render(models, (460, 460), out)
    print(f"wrote {out}: " + ", ".join(f"{len(t)} triangles" for _, t, _ in models))
