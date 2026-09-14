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


def helix_axis(turns: float, segments: int, a: float = 1.0, b: float = 0.35) -> Any:
    """A helix as an explicit polyline — the case a naive Frenet frame twists."""
    points = []
    for i in range(segments + 1):
        t = turns * 2.0 * math.pi * i / segments
        points.append(pgl.Vector3(a * math.cos(t), a * math.sin(t), b * t))
    return pgl.Polyline(pgl.Point3Array(points))


def straight_axis(height: float, segments: int) -> Any:
    return pgl.Polyline(
        pgl.Point3Array(
            [pgl.Vector3(0, 0, height * i / segments) for i in range(segments + 1)]
        )
    )


def patch_net(rows: list[list[tuple[float, float, float]]]) -> Any:
    """A control net as `[u][v]`, the layout `crates/plantgl` uses.

    Upstream's two patch classes disagree about this: `NurbsPatch::getPointAt`
    reads `getAt(u, v)` while `BezierPatch::getPointAt` reads `getAt(v, u)`, so
    the same `Point4Matrix` means transposed things to them. The port picks
    `NurbsPatch`'s reading for both, and `bezier_net` below transposes on the
    way into upstream so the two sides describe the same surface. That
    transposition is itself part of what the harness checks: if upstream ever
    made the two agree, the Bézier patch cases would stop matching.
    """
    return pgl.Point4Matrix(
        [[pgl.Vector4(x, y, z, 1.0) for (x, y, z) in row] for row in rows]
    )


def bezier_net(rows: list[list[tuple[float, float, float]]]) -> Any:
    transposed = [list(column) for column in zip(*rows)]
    return patch_net(transposed)


# A 4x4 control net with a bump in the middle, used by both patch cases.
PATCH_ROWS = [
    [(0.0, 0.0, 0.0), (0.0, 1.0, 0.8), (0.0, 2.0, -0.2), (0.0, 3.0, 0.0)],
    [(1.0, 0.0, 0.5), (1.0, 1.0, 2.0), (1.0, 2.0, 1.0), (1.0, 3.0, -0.4)],
    [(2.0, 0.0, -0.3), (2.0, 1.0, 0.9), (2.0, 2.0, 1.4), (2.0, 3.0, 0.2)],
    [(3.0, 0.0, 0.0), (3.0, 1.0, -0.6), (3.0, 2.0, 0.3), (3.0, 3.0, 0.0)],
]


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
    # --- Phase C (#19): patches and the generalized cylinder ----------------
    #
    # A patch's `UStride` counts samples where a curve's `Stride` counts
    # segments, so `s` here is a point count and the mesh is (s-1)^2 quads.
    "bezier_patch_bump": lambda s, t: pgl.BezierPatch(bezier_net(PATCH_ROWS), s, s),
    "nurbs_patch_bump": lambda s, t: pgl.NurbsPatch(
        patch_net(PATCH_ROWS), 3, 3, None, None, s, s
    ),
    # A straight axis is where the port's rotation-minimising frames and
    # upstream's projection frames provably coincide, so these must match
    # vertex for vertex.
    "extrusion_straight": lambda s, t: pgl.Extrusion(
        straight_axis(2.0, 8), pgl.Polyline2D.Circle(0.5, s)
    ),
    "extrusion_straight_solid": lambda s, t: pgl.Extrusion(
        straight_axis(2.0, 8), pgl.Polyline2D.Circle(0.5, s), None, None, None, True
    ),
    # An open cross-section: no seam to stitch, one fewer quad per ring.
    "extrusion_open_section": lambda s, t: pgl.Extrusion(
        straight_axis(2.0, 6),
        profile_polyline([(1.0, 0.0), (0.5, 0.8), (-0.5, 0.8), (-1.0, 0.0)]),
    ),
    # A tapering sweep, which exercises the scale list and its interpolation.
    "extrusion_tapered": lambda s, t: pgl.Extrusion(
        straight_axis(3.0, 8),
        pgl.Polyline2D.Circle(1.0, s),
        pgl.Point2Array([pgl.Vector2(1.0, 1.0), pgl.Vector2(0.25, 0.25)]),
    ),
    # A twisting sweep: `orientation` is applied in the cross-section plane,
    # and upstream's rotation matrix turns it clockwise.
    "extrusion_twisted": lambda s, t: pgl.Extrusion(
        straight_axis(2.0, 8),
        profile_polyline([(1.0, 0.0), (0.3, 0.6), (-1.0, 0.0), (0.3, -0.6), (1.0, 0.0)]),
        None,
        pgl.RealArray([0.0, math.pi / 3]),
    ),
    # The frame divergence, isolated: a helix has torsion, so this is the one
    # case where the two frame chains do not agree. The Rust side compares it
    # as a divergence with a derived bound rather than skipping it.
    "extrusion_helix": lambda s, t: pgl.Extrusion(
        helix_axis(2.0, 64), pgl.Polyline2D.Circle(0.15, s)
    ),
}


# --- Phase D (#20): the turtle ------------------------------------------------
#
# A turtle case is a *program*, not a geometry: the same command sequence is
# driven through upstream's `PglTurtle` and through the port's, and what is
# compared is the scene each produced — how many shapes, of what kinds, with
# what meshes — plus the frame the turtle ended in.
#
# The section resolution is pinned so both sides sweep the same number of
# facets; it is the turtle's own knob, not the discretizer's.
TURTLE_SECTION_RESOLUTION = 8


def square_section() -> Any:
    """A closed square profile — the mint-stem cross-section."""
    return pgl.Polyline2D(
        pgl.Point2Array(
            [
                pgl.Vector2(-1.0, -1.0),
                pgl.Vector2(1.0, -1.0),
                pgl.Vector2(1.0, 1.0),
                pgl.Vector2(-1.0, 1.0),
                pgl.Vector2(-1.0, -1.0),
            ]
        )
    )


def guide_arc(radius: float = 1.0, segments: int = 64) -> Any:
    """A quarter circle starting along +Z, the heading a guide assumes."""
    points = []
    for i in range(segments + 1):
        t = (math.pi / 2) * i / segments
        points.append(
            pgl.Vector3(radius * (1.0 - math.cos(t)), 0.0, radius * math.sin(t))
        )
    return pgl.Polyline(pgl.Point3Array(points))


def _straight(t: Any) -> None:
    t.setWidth(0.1)
    t.F(1.0)


def _tapered(t: Any) -> None:
    t.setWidth(0.1)
    t.F(1.0, 0.05)
    t.F(1.0, 0.02)


def _branching(t: Any) -> None:
    t.setWidth(0.05)
    t.F(1.0)
    t.push()
    t.left(35.0)
    t.F(0.7, 0.02)
    t.pop()
    t.push()
    t.right(35.0)
    t.rollL(90.0)
    t.F(0.7, 0.02)
    t.pop()
    t.down(20.0)
    t.F(0.5, 0.03)


def _primitives(t: Any) -> None:
    t.setWidth(0.2)
    t.sphere(0.3)
    t.f(1.0)
    t.circle(0.25)
    t.f(1.0)
    t.quad(0.6, 0.2)
    t.f(1.0)
    t.box(0.6, 0.2)


def _generalized_cylinder(t: Any) -> None:
    t.setWidth(0.06)
    t.startGC()
    for _ in range(20):
        t.left(3.0)
        t.F(0.1)
    t.stopGC()


def _generalized_cylinder_branch(t: Any) -> None:
    t.setWidth(0.05)
    t.startGC()
    t.F(0.5)
    t.push()
    t.left(40.0)
    t.F(0.4)
    t.F(0.4)
    t.pop()
    t.F(0.5)
    t.stopGC()


def _polygon(t: Any) -> None:
    t.startPolygon()
    t.polygonPoint()
    for _ in range(4):
        t.left(72.0)
        t.f(0.3)
        t.polygonPoint()
    t.stopPolygon(False)


def _cross_section(t: Any) -> None:
    t.setWidth(0.1)
    t.setCrossSection(square_section(), True)
    t.F(1.0)
    t.F(1.0, 0.05)


def _tropism(t: Any) -> None:
    t.setWidth(0.04)
    t.setHead(pgl.Vector3(1, 0, 0), pgl.Vector3(0, 0, 1))
    t.setTropism(0.0, 0.0, -1.0)
    t.elasticity = 0.5
    for _ in range(12):
        t.F(0.1)


def _guide(t: Any) -> None:
    t.setWidth(0.04)
    arc = guide_arc()
    length = arc.getLength()
    t.setGuide(arc, length)
    t.nF(length, length / 10.0)


def _guided_sweep(t: Any) -> None:
    """What `sweep` is: a guide, a cross-section, and `nF` along it.

    Upstream's `Turtle::sweep` is exactly that composition, but this build's
    Python bindings expose neither `sweep` (no overload accepts a `Polyline`
    path) nor the radius-varying `nF`, so the composition is driven directly
    at a constant width. The port's `sweep` and its radius profile are
    covered by `tests/turtle.rs`.
    """
    t.setWidth(0.05)
    arc = guide_arc()
    length = arc.getLength()
    t.setGuide(arc, length)
    t.setCrossSection(pgl.Polyline2D.Circle(1.0, TURTLE_SECTION_RESOLUTION), True)
    t.nF(length, length / 8.0)


def _plant(t: Any) -> None:
    """A plant-scale program: a swept trunk, branches, leaves, gravitropism."""
    t.setWidth(0.08)
    t.setTropism(0.0, 0.0, -1.0)
    t.elasticity = 0.15
    t.startGC()
    for i in range(6):
        t.F(0.3, 0.08 - 0.01 * i)
        t.rollL(60.0)
    t.stopGC()
    for i in range(3):
        t.push()
        t.left(40.0)
        t.setWidth(0.03)
        t.startGC()
        t.F(0.25)
        t.F(0.25)
        t.stopGC()
        t.push()
        t.down(30.0)
        t.surface("l", 0.4)
        t.pop()
        t.pop()
        t.rollL(120.0)
        t.f(0.1)


TURTLE_CASES: dict[str, Callable[[Any], None]] = {
    "turtle_straight": _straight,
    "turtle_tapered": _tapered,
    "turtle_branching": _branching,
    "turtle_primitives": _primitives,
    "turtle_gc": _generalized_cylinder,
    "turtle_gc_branch": _generalized_cylinder_branch,
    "turtle_polygon": _polygon,
    "turtle_cross_section": _cross_section,
    "turtle_tropism": _tropism,
    "turtle_guide": _guide,
    "turtle_guided_sweep": _guided_sweep,
    "turtle_plant": _plant,
}


def leaf_geometry(geometry: Any) -> Any:
    """The shape under however many transformations placed it.

    Both sides wrap a drawn primitive in the transformations that put it where
    the turtle stood, and neither side's wrapping is what this harness is
    about: what matters is that the same *kind* of shape came out, with the
    same mesh in the same place.
    """
    while hasattr(geometry, "geometry") and not isinstance(geometry, pgl.Group):
        geometry = geometry.geometry
    return geometry


def measure_turtle(program: Callable[[Any], None]) -> dict[str, Any]:
    """Run one turtle program upstream and record what it drew."""
    turtle = pgl.PglTurtle()
    turtle.sectionResolution = TURTLE_SECTION_RESOLUTION
    turtle.setDefaultCrossSection()
    program(turtle)
    turtle.stop()
    scene = turtle.getScene()

    shapes: list[dict[str, Any]] = []
    total_triangles = 0
    total_area = 0.0
    for shape in scene:
        geometry = shape.geometry
        kind = type(leaf_geometry(geometry)).__name__

        discretizer = pgl.Discretizer()
        if not geometry.apply(discretizer):
            raise RuntimeError(f"upstream failed to discretise a {kind}")
        mesh = discretizer.result

        triangles = pgl.tesselate(geometry)
        area = 0.0
        for i in range(triangles.indexListSize()):
            index = triangles.indexAt(i)
            corners = [triangles.pointList[index[j]] for j in range(3)]
            area += pgl.surface(corners[0], corners[1], corners[2])

        bbox_computer = pgl.BBoxComputer(pgl.Discretizer())
        geometry.apply(bbox_computer)
        bbox = bbox_computer.boundingbox

        # The first swept ring, for the skew defect the Phase D harness found:
        # upstream crosses `Extrusion::InitialNormal` with the axis tangent
        # without orthogonalising or renormalising, so the first ring of a
        # sweep that starts after a turn is an ellipse squashed by the cosine
        # of that turn. Recorded as the spread of the ring's radii, which is
        # `(r, r)` for a circular section drawn correctly.
        first_ring = (0.0, 0.0)
        if kind == "Extrusion":
            # One ring per axis point, so the ring size follows from the mesh
            # rather than from the section resolution — a section with its own
            # stride (a square, say) has fewer points than that.
            rings = len(leaf_geometry(geometry).axis.pointList)
            ring_size = len(mesh.pointList) // max(rings, 1)
            ring = list(mesh.pointList)[:ring_size]
            if ring_size >= 3:
                centre = [
                    sum(p[axis] for p in ring) / len(ring) for axis in range(3)
                ]
                radii = [
                    math.sqrt(sum((p[axis] - centre[axis]) ** 2 for axis in range(3)))
                    for p in ring
                ]
                first_ring = (min(radii), max(radii))

        total_triangles += triangles.indexListSize()
        total_area += area
        shapes.append(
            {
                "kind": kind,
                "points": len(mesh.pointList),
                "triangles": triangles.indexListSize(),
                "area": area,
                "bbox_min": [
                    bbox.lowerLeftCorner.x,
                    bbox.lowerLeftCorner.y,
                    bbox.lowerLeftCorner.z,
                ],
                "bbox_max": [
                    bbox.upperRightCorner.x,
                    bbox.upperRightCorner.y,
                    bbox.upperRightCorner.z,
                ],
                "first_ring_min": first_ring[0],
                "first_ring_max": first_ring[1],
            }
        )

    position, heading, left, up = (
        turtle.getPosition(),
        turtle.getHeading(),
        turtle.getLeft(),
        turtle.getUp(),
    )
    return {
        "section_resolution": TURTLE_SECTION_RESOLUTION,
        "shapes": shapes,
        "shape_count": len(shapes),
        "triangles": total_triangles,
        "area": total_area,
        "final_position": [position.x, position.y, position.z],
        "final_heading": [heading.x, heading.y, heading.z],
        "final_left": [left.x, left.y, left.z],
        "final_up": [up.x, up.y, up.z],
        "final_width": turtle.getWidth(),
    }


# Curves are not meshes: a `Discretizer` reduces them to a `Polyline`, so there
# is no area, volume or face count to compare. What there is instead is the
# thing that matters — the sampled points, and the tangents at them.
CURVE_CASES: dict[str, Callable[[], Any]] = {
    "bezier_curve_cubic": lambda: pgl.BezierCurve(
        pgl.Point4Array(
            [
                pgl.Vector4(0, 0, 0, 1),
                pgl.Vector4(1, 4, -2, 1),
                pgl.Vector4(3, -1, 2, 1),
                pgl.Vector4(5, 2, 0, 1),
            ]
        )
    ),
    # Weighted control points: upstream's Bézier *point* is rational and
    # correct, and its *tangent* is not (see `TANGENT_IS_COMPARABLE`).
    "bezier_curve_rational": lambda: pgl.BezierCurve(
        pgl.Point4Array(
            [
                pgl.Vector4(1, 0, 0, 1),
                pgl.Vector4(1, 1, 0, 0.5),
                pgl.Vector4(0, 1, 1, 2.0),
                pgl.Vector4(-1, 0, 1, 1),
            ]
        )
    ),
    "nurbs_curve_cubic": lambda: pgl.NurbsCurve(
        pgl.Point4Array(
            [
                pgl.Vector4(0, 0, 0, 1),
                pgl.Vector4(1, 2, 0, 1),
                pgl.Vector4(2, -1, 1, 1),
                pgl.Vector4(3, 1, 2, 1),
                pgl.Vector4(4, 0, 0, 1),
                pgl.Vector4(5, 2, 1, 1),
            ]
        ),
        3,
    ),
    # An explicit, non-uniform knot vector: the span search is what this checks.
    "nurbs_curve_knots": lambda: pgl.NurbsCurve(
        pgl.Point4Array(
            [
                pgl.Vector4(0, 0, 0, 1),
                pgl.Vector4(1, 3, 0, 1),
                pgl.Vector4(2, 0, 2, 1),
                pgl.Vector4(4, 1, 0, 1),
                pgl.Vector4(5, -1, 1, 1),
            ]
        ),
        2,
        pgl.RealArray([0.0, 0.0, 0.0, 0.3, 0.75, 1.0, 1.0, 1.0]),
    ),
    "bezier_curve_2d": lambda: pgl.BezierCurve2D(
        pgl.Point3Array(
            [
                pgl.Vector3(0, 0, 1),
                pgl.Vector3(1, 2, 1),
                pgl.Vector3(3, -1, 1),
                pgl.Vector3(4, 0, 1),
            ]
        )
    ),
    # The nine-point rational circle, which is the T8.6 acceptance case.
    "nurbs_curve_2d_circle": lambda: pgl.NurbsCurve2D(
        pgl.Point3Array(
            [
                pgl.Vector3(1, 0, 1),
                pgl.Vector3(1, 1, math.sqrt(2) / 2),
                pgl.Vector3(0, 1, 1),
                pgl.Vector3(-1, 1, math.sqrt(2) / 2),
                pgl.Vector3(-1, 0, 1),
                pgl.Vector3(-1, -1, math.sqrt(2) / 2),
                pgl.Vector3(0, -1, 1),
                pgl.Vector3(1, -1, math.sqrt(2) / 2),
                pgl.Vector3(1, 0, 1),
            ]
        ),
        2,
        pgl.RealArray([0.0, 0.0, 0.0, 0.25, 0.25, 0.5, 0.5, 0.75, 0.75, 1.0, 1.0, 1.0]),
    ),
}

# `BezierCurve::getTangentAt` differences the stored control points and calls
# `project()` on the result, which divides by a difference of *weights* instead
# of applying the quotient rule. For weight-1 curves the difference has w = 0
# and upstream's answer is the right one; for a rational Bézier it is not a
# tangent at all. `NurbsCurve::getTangentAt` goes through `deriveAt`, which is
# correct in both cases. The port always applies the quotient rule, so tangents
# are compared everywhere except upstream's broken case — where the Rust side
# instead asserts that upstream still disagrees.
TANGENT_IS_COMPARABLE = {name: name != "bezier_curve_rational" for name in CURVE_CASES}

# The parameters every curve is sampled at, as a fraction of its knot range.
CURVE_SAMPLES = 33

# The stride every curve is discretised at, so the polyline point count is
# fixed rather than inherited from a default that might change upstream.
CURVE_STRIDE = 24


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
    # Faces whose normal points back towards the middle of the mesh. On a
    # closed, star-shaped solid that number should be zero, and where it is not
    # some face is wound the wrong way round — which upstream's own VolComputer
    # cannot see, because it sums the *absolute* tetrahedra about the centroid.
    # Recorded for every case so the Rust side can both pin the one place
    # upstream gets it wrong and confirm it is the only one.
    centroid = [
        sum(p[axis] for p in mesh.pointList) / len(mesh.pointList) for axis in range(3)
    ]
    degenerate = 0
    inward = 0
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
            continue
        normal = pgl.cross(corners[1] - corners[0], corners[2] - corners[0])
        outward = [
            sum(c[axis] for c in corners) / len(corners) - centroid[axis]
            for axis in range(3)
        ]
        if sum(normal[axis] * outward[axis] for axis in range(3)) < 0:
            inward += 1

    return {
        "points": len(mesh.pointList),
        "faces": mesh.indexListSize(),
        "triangles": triangles.indexListSize(),
        "degenerate_faces": degenerate,
        "inward_faces": inward,
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


def measure_curve(name: str, curve: Any) -> dict[str, Any]:
    """Sampled points and tangents, plus the polyline the discretizer makes."""
    curve.stride = CURVE_STRIDE
    first, last = curve.firstKnot, curve.lastKnot
    two_d = isinstance(curve, pgl.Curve2D)

    def flatten(vector: Any) -> list[float]:
        return [vector.x, vector.y] if two_d else [vector.x, vector.y, vector.z]

    samples: list[float] = []
    tangents: list[float] = []
    for i in range(CURVE_SAMPLES):
        u = first + (last - first) * i / (CURVE_SAMPLES - 1)
        samples += flatten(curve.getPointAt(u))
        tangents += flatten(curve.getTangentAt(u))

    discretizer = pgl.Discretizer()
    if not curve.apply(discretizer):
        raise RuntimeError("upstream failed to discretise the curve")
    polyline = discretizer.result

    return {
        "dimensions": 2 if two_d else 3,
        "first_knot": first,
        "last_knot": last,
        "stride": CURVE_STRIDE,
        "sample_count": CURVE_SAMPLES,
        "samples": samples,
        "tangents": tangents,
        "tangent_is_comparable": TANGENT_IS_COMPARABLE[name],
        # `BezierCurve::getTangentAt` special-cases both ends and gets both
        # wrong: at u = 0 it returns the *normalised* P1 - P0, and at u = 1 the
        # raw P(n) - P(n-1) without the factor of the degree. Its interior
        # branch is the true derivative, so upstream's own tangent is
        # discontinuous at both ends of every Bézier curve. The Rust side
        # compares the interior and pins the two endpoints as defects.
        # An exact class check, not `isinstance`: upstream derives NurbsCurve
        # from BezierCurve (and NurbsCurve2D from BezierCurve2D) but overrides
        # `getTangentAt` with the correct `deriveAt`, so the NURBS classes do
        # not carry the defect their base class does.
        "tangent_endpoints_comparable": type(curve).__name__
        not in ("BezierCurve", "BezierCurve2D"),
        # The discretizer always produces a 3D polyline; a 2D curve is embedded
        # at z = 0.
        "discretized": [c for p in polyline.pointList for c in (p.x, p.y, p.z)],
        "length": curve.getLength(),
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

    turtles: dict[str, Any] = {}
    for name, program in TURTLE_CASES.items():
        if args.only and name != args.only:
            continue
        try:
            turtles[name] = measure_turtle(program)
        except Exception as exc:  # noqa: BLE001 - report, do not mask
            failures[name] = f"{type(exc).__name__}: {exc}"

    curves: dict[str, Any] = {}
    for name, build_curve in CURVE_CASES.items():
        if args.only and name != args.only:
            continue
        try:
            curves[name] = measure_curve(name, build_curve())
        except Exception as exc:  # noqa: BLE001 - report, do not mask
            failures[name] = f"{type(exc).__name__}: {exc}"

    document = {
        "_comment": (
            "Generated by tools/differential/upstream_measure.py from upstream "
            "PlantGL. Do not hand-edit: regenerate it. See "
            "tools/differential/README.md."
        ),
        "plantgl_version": str(version),
        "slice_counts": list(SLICE_COUNTS),
        "cases": results,
        "curves": curves,
        "turtles": turtles,
    }
    if failures:
        document["failures"] = failures

    with open(args.out, "w", encoding="utf-8") as handle:
        json.dump(document, handle, indent=2, sort_keys=True)
        handle.write("\n")

    print(
        f"wrote {len(results)} measurements, {len(curves)} curves and "
        f"{len(turtles)} turtle programs to {args.out}"
    )
    for key, why in failures.items():
        print(f"  FAILED {key}: {why}", file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
