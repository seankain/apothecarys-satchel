#!/usr/bin/env python3
"""Measure a fixed set of PlantGL primitives with *upstream* PlantGL.

Emits `reference.json`, which `crates/plantgl/tests/differential.rs` checks the
Rust port against. Run it whenever the case list changes or upstream is
rebased; the committed JSON is what CI actually gates on, so this script is
**never** a build dependency and needs no Python at test time.

    conda run -n plantgl-diff python tools/differential/upstream_measure.py \
        --out crates/plantgl/tests/reference.json

See README.md for the conda environment.

This file drives PlantGL but contains no translated PlantGL code. It lives
outside `crates/plantgl` and is MIT, like the rest of the workspace.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from typing import Any, Callable

try:
    import openalea.plantgl.all as pgl
except ImportError:  # pragma: no cover - the script is useless without it
    sys.exit(
        "openalea.plantgl is not importable. Create the environment first:\n"
        "  conda env create -f tools/differential/environment.yml\n"
        "  conda run -n plantgl-diff python tools/differential/upstream_measure.py"
    )


# The slice counts every case is measured at. Two, as T8.5 asks: one coarse
# enough that the polygonal error is obvious and one fine enough that it is
# not, so a discrepancy that scales with density is distinguishable from a
# constant offset.
SLICE_COUNTS = (8, 32)


def profile_polyline(points: list[tuple[float, float]]) -> Any:
    return pgl.Polyline2D(pgl.Point2Array([pgl.Vector2(x, y) for x, y in points]))


# Each entry builds one primitive at a given (slices, stacks). The names match
# the `case` strings in `crates/plantgl/tests/differential.rs`; keep the two in
# step.
CASES: dict[str, Callable[[int, int], Any]] = {
    # `Box` has no density at all, so it is measured once per slice count and
    # must come back identical both times — a useful control on the harness.
    "box_unit": lambda s, t: pgl.Box(pgl.Vector3(0.5, 0.5, 0.5)),
    "box_oblong": lambda s, t: pgl.Box(pgl.Vector3(1.0, 2.0, 3.0)),
    "sphere_unit": lambda s, t: pgl.Sphere(1.0, s, t),
    "sphere_small": lambda s, t: pgl.Sphere(0.25, s, t),
    "cone_solid": lambda s, t: pgl.Cone(1.0, 2.0, True, s),
    "cone_open": lambda s, t: pgl.Cone(1.0, 2.0, False, s),
    "cylinder_solid": lambda s, t: pgl.Cylinder(1.0, 2.0, True, s),
    "cylinder_open": lambda s, t: pgl.Cylinder(1.0, 2.0, False, s),
    "frustum_solid": lambda s, t: pgl.Frustum(2.0, 3.0, 0.5, True, s),
    "frustum_open": lambda s, t: pgl.Frustum(2.0, 3.0, 0.5, False, s),
    "frustum_cone": lambda s, t: pgl.Frustum(1.0, 2.0, 0.0, True, s),
    "disc_unit": lambda s, t: pgl.Disc(1.0, s),
    # A radius other than 1, so `2πr` and `2πr²` are distinguishable in the
    # data — it is what tells the circumference bug in upstream's
    # `SurfComputer::process(Disc*)` apart from a mere factor of two.
    "disc_r3": lambda s, t: pgl.Disc(3.0, s),
    "paraboloid_solid": lambda s, t: pgl.Paraboloid(1.0, 2.0, 2.0, True, s, t),
    "paraboloid_sharp": lambda s, t: pgl.Paraboloid(1.0, 2.0, 0.5, True, s, t),
    # A profile that stays clear of the axis: an open tube, so no pole
    # collapsing is involved and the port should match upstream exactly.
    "revolution_tube": lambda s, t: pgl.Revolution(
        profile_polyline([(2.0, 0.0), (1.5, 1.0), (1.0, 3.0)]), s
    ),
    # A profile that touches the axis at both ends. The port collapses the
    # poles and upstream does not, so the triangle counts differ by design;
    # see `pole_collapse` in the Rust test.
    "revolution_closed": lambda s, t: pgl.Revolution(
        profile_polyline([(0.0, 0.0), (1.0, 1.0), (1.0, 2.0), (0.0, 3.0)]), s
    ),
    "elevation_grid_flat": lambda s, t: pgl.ElevationGrid(
        pgl.RealArray2([[0.0] * 5 for _ in range(5)]), 1.0, 1.0
    ),
    "elevation_grid_bump": lambda s, t: pgl.ElevationGrid(
        pgl.RealArray2(
            [
                [math.sin(i * 0.7) * math.cos(j * 0.5) for i in range(6)]
                for j in range(4)
            ]
        ),
        0.5,
        2.0,
    ),
}


# Upstream's SurfComputer and VolComputer do NOT always measure the mesh: for
# some primitives they return the closed form of the *ideal* surface, so the
# number is density-independent and a discretised measure can only converge
# towards it. The rest fall through to the discretizer and are directly
# comparable, mesh to mesh.
#
# The two computers do not agree on which is which — `SurfComputer::process(
# Paraboloid*)` discretises while `VolComputer::process(Paraboloid*)` uses
# `V = π h r² s / (s + 2)` with its `GEOM_DISCRETIZE` commented out — so the
# split is recorded per measure rather than per primitive.
ANALYTIC_AREA: set[str] = {"Box", "Cone", "Cylinder", "Disc", "Frustum", "Sphere"}
ANALYTIC_VOLUME: set[str] = ANALYTIC_AREA | {"Paraboloid", "ElevationGrid"}


def measure(geometry: Any) -> dict[str, Any]:
    """Everything the Rust side can compare: bbox, area, volume, counts."""
    discretizer = pgl.Discretizer()
    if not geometry.apply(discretizer):
        raise RuntimeError("upstream failed to discretise the geometry")
    mesh = discretizer.result

    bbox_computer = pgl.BBoxComputer(pgl.Discretizer())
    geometry.apply(bbox_computer)
    bbox = bbox_computer.boundingbox

    surf = pgl.SurfComputer(pgl.Discretizer())
    geometry.apply(surf)

    vol = pgl.VolComputer(pgl.Discretizer())
    geometry.apply(vol)

    triangles = pgl.tesselate(geometry)

    # Count the zero-area triangles upstream emits where a swept profile meets
    # the axis of revolution. The port collapses those poles instead, so this
    # is exactly the difference in face count the Rust side should expect —
    # recorded rather than assumed.
    degenerate = 0
    for i in range(mesh.indexListSize()):
        index = mesh.indexAt(i)
        if len(index) < 3:
            continue
        corners = [mesh.pointList[index[j]] for j in range(len(index))]
        area = sum(
            pgl.surface(corners[0], corners[j], corners[j + 1])
            for j in range(1, len(corners) - 1)
        )
        if area < 1e-9:
            degenerate += 1

    return {
        "points": len(mesh.pointList),
        "faces": mesh.indexListSize(),
        "triangles": triangles.indexListSize(),
        "degenerate_faces": degenerate,
        # `solid` decides whether upstream's VolComputer reports anything at
        # all, so it is part of the comparison rather than an aside.
        "solid": bool(mesh.solid),
        "ccw": bool(mesh.ccw),
        "bbox_min": [bbox.lowerLeftCorner.x, bbox.lowerLeftCorner.y, bbox.lowerLeftCorner.z],
        "bbox_max": [
            bbox.upperRightCorner.x,
            bbox.upperRightCorner.y,
            bbox.upperRightCorner.z,
        ],
        "area": surf.surface,
        "volume": vol.volume,
        # Whether each number above describes the sampled mesh or the ideal
        # surface. The Rust side compares tightly when it is the mesh and
        # checks convergence when it is the ideal.
        "area_measures_mesh": type(geometry).__name__ not in ANALYTIC_AREA,
        "volume_measures_mesh": type(geometry).__name__ not in ANALYTIC_VOLUME,
        # The port sums signed tetrahedra about the origin; upstream sums
        # absolute ones about the centroid. They agree on star-shaped solids
        # and not otherwise, so the honest thing is to record upstream's
        # number and let the Rust side say which cases it expects to match.
        "volume_method": "upstream_abs_about_centroid",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out",
        default="crates/plantgl/tests/reference.json",
        help="where to write the reference JSON",
    )
    parser.add_argument(
        "--only",
        default=None,
        help="measure just this case, for debugging one discrepancy",
    )
    args = parser.parse_args()

    version = getattr(pgl, "PGL_VERSION_STR", None) or getattr(
        pgl, "getPGLVersionString", lambda: "unknown"
    )()

    results: dict[str, Any] = {}
    failures: dict[str, str] = {}

    for name, build in CASES.items():
        if args.only and name != args.only:
            continue
        for slices in SLICE_COUNTS:
            key = f"{name}@{slices}"
            try:
                results[key] = measure(build(slices, slices))
            except Exception as exc:  # noqa: BLE001 - report, do not mask
                failures[key] = f"{type(exc).__name__}: {exc}"

    document = {
        "_comment": (
            "Generated by tools/differential/upstream_measure.py from upstream "
            "PlantGL. Do not hand-edit: regenerate it. See "
            "tools/differential/README.md."
        ),
        "plantgl_version": str(version),
        "slice_counts": list(SLICE_COUNTS),
        "cases": results,
    }
    if failures:
        document["failures"] = failures

    with open(args.out, "w", encoding="utf-8") as handle:
        json.dump(document, handle, indent=2, sort_keys=True)
        handle.write("\n")

    print(f"wrote {len(results)} measurements to {args.out}")
    for key, why in failures.items():
        print(f"  FAILED {key}: {why}", file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
