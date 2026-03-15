use glam::Vec3;
use pathfinding::prelude::astar;
use serde::{Deserialize, Serialize};

/// A convex polygon in the navigation mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavPolygon {
    /// Indices into the NavMesh vertex array.
    pub vertices: Vec<usize>,
    /// Indices of neighboring polygons (one per edge, None if boundary).
    pub neighbors: Vec<Option<usize>>,
}

/// A navigation mesh for pathfinding on walkable surfaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavMesh {
    pub vertices: Vec<Vec3>,
    pub polygons: Vec<NavPolygon>,
}

impl NavMesh {
    pub fn new(vertices: Vec<Vec3>, polygons: Vec<NavPolygon>) -> Self {
        Self { vertices, polygons }
    }

    /// Get the centroid of a polygon.
    pub fn polygon_center(&self, poly_idx: usize) -> Vec3 {
        let poly = &self.polygons[poly_idx];
        let sum: Vec3 = poly
            .vertices
            .iter()
            .map(|&vi| self.vertices[vi])
            .sum();
        sum / poly.vertices.len() as f32
    }

    /// Get the vertices of an edge shared between two adjacent polygons.
    /// Returns (left_vertex, right_vertex) looking from `from_poly` toward `to_poly`.
    pub fn shared_edge(&self, from_poly: usize, to_poly: usize) -> Option<(Vec3, Vec3)> {
        let poly_a = &self.polygons[from_poly];
        let poly_b = &self.polygons[to_poly];

        for i in 0..poly_a.vertices.len() {
            let a0 = poly_a.vertices[i];
            let a1 = poly_a.vertices[(i + 1) % poly_a.vertices.len()];
            for j in 0..poly_b.vertices.len() {
                let b0 = poly_b.vertices[j];
                let b1 = poly_b.vertices[(j + 1) % poly_b.vertices.len()];
                // Shared edge: vertices match in reverse order
                if (a0 == b1 && a1 == b0) || (a0 == b0 && a1 == b1) {
                    return Some((self.vertices[a0], self.vertices[a1]));
                }
            }
        }
        None
    }

    /// Find which polygon contains the given point (projected onto XZ plane).
    pub fn find_polygon(&self, point: Vec3) -> Option<usize> {
        for (i, poly) in self.polygons.iter().enumerate() {
            if self.point_in_polygon(point, poly) {
                return Some(i);
            }
        }
        None
    }

    /// Check if a point is on the navmesh.
    pub fn is_walkable(&self, point: Vec3) -> bool {
        self.find_polygon(point).is_some()
    }

    /// Test if a point is inside a convex polygon (XZ plane projection).
    fn point_in_polygon(&self, point: Vec3, poly: &NavPolygon) -> bool {
        let n = poly.vertices.len();
        if n < 3 {
            return false;
        }

        let mut sign = None;
        for i in 0..n {
            let v0 = self.vertices[poly.vertices[i]];
            let v1 = self.vertices[poly.vertices[(i + 1) % n]];
            let cross = (v1.x - v0.x) * (point.z - v0.z) - (v1.z - v0.z) * (point.x - v0.x);
            let s = cross >= 0.0;
            match sign {
                None => sign = Some(s),
                Some(prev) if prev != s => return false,
                _ => {}
            }
        }
        true
    }

    /// Find a path from `start` to `end` using A* over polygon graph + funnel smoothing.
    pub fn find_path(&self, start: Vec3, end: Vec3) -> Option<Vec<Vec3>> {
        let start_poly = self.find_polygon(start)?;
        let end_poly = self.find_polygon(end)?;

        if start_poly == end_poly {
            return Some(vec![start, end]);
        }

        // A* over polygon graph
        let poly_path = astar(
            &start_poly,
            |&current| {
                self.polygons[current]
                    .neighbors
                    .iter()
                    .filter_map(|n| *n)
                    .map(|neighbor| {
                        let cost = self.polygon_center(current)
                            .distance(self.polygon_center(neighbor));
                        (neighbor, (cost * 1000.0) as u32)
                    })
                    .collect::<Vec<_>>()
            },
            |&current| {
                let cost = self.polygon_center(current).distance(end);
                (cost * 1000.0) as u32
            },
            |&current| current == end_poly,
        )?;

        let poly_path = poly_path.0;

        // Apply funnel algorithm for path smoothing
        self.funnel_smooth(start, end, &poly_path)
    }

    /// Funnel algorithm (simple string-pulling) for path smoothing.
    fn funnel_smooth(&self, start: Vec3, end: Vec3, poly_path: &[usize]) -> Option<Vec<Vec3>> {
        if poly_path.len() <= 1 {
            return Some(vec![start, end]);
        }

        // Collect portals (shared edges between consecutive polygons)
        let mut portals: Vec<(Vec3, Vec3)> = Vec::new();
        for i in 0..poly_path.len() - 1 {
            if let Some(edge) = self.shared_edge(poly_path[i], poly_path[i + 1]) {
                portals.push(edge);
            } else {
                // Fallback: no shared edge found, use centroids
                return Some(self.centroid_path(start, end, poly_path));
            }
        }

        // Simple funnel algorithm
        let mut path = vec![start];
        let mut apex = start;
        let mut left = start;
        let mut right = start;
        let mut left_idx: usize = 0;
        let mut right_idx: usize = 0;

        for i in 0..portals.len() {
            let (portal_left, portal_right) = portals[i];

            // Update right
            if triangle_area_2d(apex, right, portal_right) <= 0.0 {
                if apex == right || triangle_area_2d(apex, left, portal_right) > 0.0 {
                    right = portal_right;
                    right_idx = i;
                } else {
                    // Right over left, add left to path
                    path.push(left);
                    apex = left;
                    right = apex;
                    left_idx += 1;
                    right_idx = left_idx;
                    // Restart from left_idx
                    if left_idx < portals.len() {
                        left = portals[left_idx].0;
                        right = portals[left_idx].1;
                    }
                    continue;
                }
            }

            // Update left
            if triangle_area_2d(apex, left, portal_left) >= 0.0 {
                if apex == left || triangle_area_2d(apex, right, portal_left) < 0.0 {
                    left = portal_left;
                    left_idx = i;
                } else {
                    // Left over right, add right to path
                    path.push(right);
                    apex = right;
                    left = apex;
                    right_idx += 1;
                    left_idx = right_idx;
                    if right_idx < portals.len() {
                        left = portals[right_idx].0;
                        right = portals[right_idx].1;
                    }
                    continue;
                }
            }

            // Suppress unused variable warnings - indices are used for tracking
            let _ = left_idx;
            let _ = right_idx;
        }

        path.push(end);

        // Deduplicate consecutive identical points
        path.dedup_by(|a, b| a.distance(*b) < 0.001);

        Some(path)
    }

    /// Fallback: path through polygon centroids.
    fn centroid_path(&self, start: Vec3, end: Vec3, poly_path: &[usize]) -> Vec<Vec3> {
        let mut path = vec![start];
        for &poly_idx in &poly_path[1..poly_path.len() - 1] {
            path.push(self.polygon_center(poly_idx));
        }
        path.push(end);
        path
    }
}

/// 2D triangle area (XZ plane), positive if counter-clockwise.
fn triangle_area_2d(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    (b.x - a.x) * (c.z - a.z) - (c.x - a.x) * (b.z - a.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a simple 2-polygon navmesh:
    /// ```text
    /// v0---v1---v2
    ///  |  P0  |  P1  |
    /// v3---v4---v5
    /// ```
    fn make_two_quad_mesh() -> NavMesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0), // v0
            Vec3::new(5.0, 0.0, 0.0), // v1
            Vec3::new(10.0, 0.0, 0.0), // v2
            Vec3::new(0.0, 0.0, 5.0), // v3
            Vec3::new(5.0, 0.0, 5.0), // v4
            Vec3::new(10.0, 0.0, 5.0), // v5
        ];
        let polygons = vec![
            NavPolygon {
                vertices: vec![0, 1, 4, 3],
                neighbors: vec![None, Some(1), None, None],
            },
            NavPolygon {
                vertices: vec![1, 2, 5, 4],
                neighbors: vec![None, None, None, Some(0)],
            },
        ];
        NavMesh::new(vertices, polygons)
    }

    /// Create an L-shaped navmesh with 3 polygons:
    /// ```text
    /// v0---v1---v2
    ///  |  P0  |  P1  |
    /// v3---v4---v5
    ///       |  P2  |
    ///      v6---v7
    /// ```
    fn make_l_shaped_mesh() -> NavMesh {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0), // v0
            Vec3::new(5.0, 0.0, 0.0), // v1
            Vec3::new(10.0, 0.0, 0.0), // v2
            Vec3::new(0.0, 0.0, 5.0), // v3
            Vec3::new(5.0, 0.0, 5.0), // v4
            Vec3::new(10.0, 0.0, 5.0), // v5
            Vec3::new(5.0, 0.0, 10.0), // v6
            Vec3::new(10.0, 0.0, 10.0), // v7
        ];
        let polygons = vec![
            NavPolygon {
                vertices: vec![0, 1, 4, 3],
                neighbors: vec![None, Some(1), None, None],
            },
            NavPolygon {
                vertices: vec![1, 2, 5, 4],
                neighbors: vec![None, None, Some(2), Some(0)],
            },
            NavPolygon {
                vertices: vec![4, 5, 7, 6],
                neighbors: vec![Some(1), None, None, None],
            },
        ];
        NavMesh::new(vertices, polygons)
    }

    #[test]
    fn test_polygon_center() {
        let mesh = make_two_quad_mesh();
        let center = mesh.polygon_center(0);
        assert!((center.x - 2.5).abs() < 0.01);
        assert!((center.z - 2.5).abs() < 0.01);
    }

    #[test]
    fn test_is_walkable() {
        let mesh = make_two_quad_mesh();
        // Inside polygon 0
        assert!(mesh.is_walkable(Vec3::new(2.5, 0.0, 2.5)));
        // Inside polygon 1
        assert!(mesh.is_walkable(Vec3::new(7.5, 0.0, 2.5)));
        // Outside mesh
        assert!(!mesh.is_walkable(Vec3::new(-1.0, 0.0, 2.5)));
        assert!(!mesh.is_walkable(Vec3::new(5.0, 0.0, 6.0)));
    }

    #[test]
    fn test_find_polygon() {
        let mesh = make_two_quad_mesh();
        assert_eq!(mesh.find_polygon(Vec3::new(2.5, 0.0, 2.5)), Some(0));
        assert_eq!(mesh.find_polygon(Vec3::new(7.5, 0.0, 2.5)), Some(1));
        assert_eq!(mesh.find_polygon(Vec3::new(-1.0, 0.0, 2.5)), None);
    }

    #[test]
    fn test_find_path_same_polygon() {
        let mesh = make_two_quad_mesh();
        let start = Vec3::new(1.0, 0.0, 1.0);
        let end = Vec3::new(4.0, 0.0, 4.0);
        let path = mesh.find_path(start, end).unwrap();
        assert_eq!(path.len(), 2);
        assert_eq!(path[0], start);
        assert_eq!(path[1], end);
    }

    #[test]
    fn test_find_path_adjacent_polygons() {
        let mesh = make_two_quad_mesh();
        let start = Vec3::new(2.0, 0.0, 2.5);
        let end = Vec3::new(8.0, 0.0, 2.5);
        let path = mesh.find_path(start, end);
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.len() >= 2);
        assert_eq!(path[0], start);
        assert_eq!(*path.last().unwrap(), end);
    }

    #[test]
    fn test_find_path_across_l_shape() {
        let mesh = make_l_shaped_mesh();
        let start = Vec3::new(2.0, 0.0, 2.5);
        let end = Vec3::new(7.5, 0.0, 7.5);
        let path = mesh.find_path(start, end);
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.len() >= 2);
        assert_eq!(path[0], start);
        assert_eq!(*path.last().unwrap(), end);
    }

    #[test]
    fn test_find_path_unreachable() {
        // No neighbor connection between the two polygons
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(5.0, 0.0, 0.0),
            Vec3::new(5.0, 0.0, 5.0),
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(20.0, 0.0, 0.0),
            Vec3::new(25.0, 0.0, 0.0),
            Vec3::new(25.0, 0.0, 5.0),
            Vec3::new(20.0, 0.0, 5.0),
        ];
        let polygons = vec![
            NavPolygon { vertices: vec![0, 1, 2, 3], neighbors: vec![None, None, None, None] },
            NavPolygon { vertices: vec![4, 5, 6, 7], neighbors: vec![None, None, None, None] },
        ];
        let mesh = NavMesh::new(vertices, polygons);
        let path = mesh.find_path(Vec3::new(2.5, 0.0, 2.5), Vec3::new(22.5, 0.0, 2.5));
        assert!(path.is_none());
    }

    #[test]
    fn test_find_path_off_mesh() {
        let mesh = make_two_quad_mesh();
        assert!(mesh.find_path(Vec3::new(-10.0, 0.0, -10.0), Vec3::new(2.5, 0.0, 2.5)).is_none());
        assert!(mesh.find_path(Vec3::new(2.5, 0.0, 2.5), Vec3::new(-10.0, 0.0, -10.0)).is_none());
    }

    #[test]
    fn test_shared_edge() {
        let mesh = make_two_quad_mesh();
        let edge = mesh.shared_edge(0, 1);
        assert!(edge.is_some());
        let (a, b) = edge.unwrap();
        // Shared edge between P0 and P1 is v1-v4 (indices 1 and 4)
        let v1 = Vec3::new(5.0, 0.0, 0.0);
        let v4 = Vec3::new(5.0, 0.0, 5.0);
        assert!(
            (a.distance(v1) < 0.01 && b.distance(v4) < 0.01)
            || (a.distance(v4) < 0.01 && b.distance(v1) < 0.01)
        );
    }

    #[test]
    fn test_navmesh_serde_roundtrip() {
        let mesh = make_two_quad_mesh();
        let json = serde_json::to_string(&mesh).unwrap();
        let deserialized: NavMesh = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.vertices.len(), mesh.vertices.len());
        assert_eq!(deserialized.polygons.len(), mesh.polygons.len());
    }

    #[test]
    fn test_path_stays_on_mesh() {
        let mesh = make_l_shaped_mesh();
        let start = Vec3::new(2.0, 0.0, 2.5);
        let end = Vec3::new(7.5, 0.0, 7.5);
        let path = mesh.find_path(start, end).unwrap();

        // All path waypoints should be within reasonable distance of the mesh
        // (the funnel algorithm might produce points on polygon edges)
        for point in &path {
            // Check that the point is not wildly off the mesh
            assert!(point.x >= -0.1 && point.x <= 10.1);
            assert!(point.z >= -0.1 && point.z <= 10.1);
        }
    }
}
