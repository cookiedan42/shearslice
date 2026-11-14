//! Octree topology and `tree.bin` encoder (port of theloderma `tree_bin.py`).
//!
//! The tree is 4-ary along every axis (up to 64 children per node); leaves are single bricks at
//! depth 0. Nodes are stored breadth-first: root, then each deeper level, then the leaves. Within a
//! level they follow their parents' ascending slot order (`slot = lx + 4*ly + 16*lz`).
//!
//! Size: `12 + 124*N` bytes = TREE (N×48) + MID (N×72) + TAIL (16 + 4×childCount).

pub struct Node {
    pub x: u32,
    pub y: u32,
    pub z: u32,
    pub depth: u32,
    pub child_mask: u64,
    /// Indices into the flat node list.
    pub children: Vec<u32>,
    pub start: u32,
    pub min: f64,
    pub max: f64,
}

/// Counts written to `index.json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeCounts {
    pub node_count: usize,
    pub children_list_count: usize,
    pub leaf_brick_count: usize,
}

/// Smallest `d` with `4^d >= max(nb)`.
pub fn max_depth_for(nb: [usize; 3]) -> u32 {
    let max_dim = nb.into_iter().max().unwrap_or(1).max(1);
    let (mut d, mut extent) = (0, 1usize);
    while extent < max_dim {
        d += 1;
        extent *= 4;
    }
    d
}

/// Build the full node list in `tree.bin` order.
///
/// `leaf_mm` is indexed `x + nbx*(y + nby*z)`. Leaves with no valid voxel (`min == +inf`) take
/// `fallback`.
pub fn build_tree(nb: [usize; 3], leaf_mm: &[(f64, f64)], fallback: (f64, f64)) -> Vec<Node> {
    let [nbx, nby, nbz] = nb;
    let max_d = max_depth_for(nb);
    let new_node = |x: u32, y: u32, z: u32, depth: u32| Node {
        x,
        y,
        z,
        depth,
        child_mask: 0,
        children: Vec::new(),
        start: 0,
        min: f64::INFINITY,
        max: f64::NEG_INFINITY,
    };

    // layers[d] = nodes at depth d in natural order.
    let mut layers: Vec<Vec<Node>> = (0..=max_d).map(|_| Vec::new()).collect();
    layers[max_d as usize].push(new_node(0, 0, 0, max_d));

    for d in (1..=max_d).rev() {
        let stride = 4usize.pow(d - 1);
        let mut next = Vec::new();
        for parent in &mut layers[d as usize] {
            let mut mask = 0u64;
            for lz in 0..4u32 {
                for ly in 0..4u32 {
                    for lx in 0..4u32 {
                        let (cx, cy, cz) =
                            (parent.x * 4 + lx, parent.y * 4 + ly, parent.z * 4 + lz);
                        if cx as usize * stride >= nbx
                            || cy as usize * stride >= nby
                            || cz as usize * stride >= nbz
                        {
                            continue;
                        }
                        mask |= 1 << (lx + 4 * ly + 16 * lz);
                        next.push(new_node(cx, cy, cz, d - 1));
                    }
                }
            }
            parent.child_mask = mask;
        }
        layers[d as usize - 1] = next;
    }

    // Flatten root-first and wire children. Children of layer d were appended in parent order, so
    // the k-th child created in layer d-1 sits at `offset[d-1] + k`.
    let mut offsets = vec![0usize; max_d as usize + 1];
    let mut acc = 0;
    for d in (0..=max_d as usize).rev() {
        offsets[d] = acc;
        acc += layers[d].len();
    }
    for d in (1..=max_d as usize).rev() {
        let mut k = 0;
        for n in &mut layers[d] {
            for _ in 0..n.child_mask.count_ones() {
                n.children.push((offsets[d - 1] + k) as u32);
                k += 1;
            }
        }
    }
    let mut nodes: Vec<Node> = layers.into_iter().rev().flatten().collect();

    // Bottom-up (min, max): children always have larger indices than their parent.
    for i in (0..nodes.len()).rev() {
        let (lo, hi) = if nodes[i].child_mask == 0 {
            let n = &nodes[i];
            let mm = leaf_mm[n.x as usize + nbx * (n.y as usize + nby * n.z as usize)];
            if mm.0 == f64::INFINITY { fallback } else { mm }
        } else {
            nodes[i]
                .children
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &c| {
                    (lo.min(nodes[c as usize].min), hi.max(nodes[c as usize].max))
                })
        };
        nodes[i].min = lo;
        nodes[i].max = hi;
    }

    let mut s = 0;
    for n in &mut nodes {
        n.start = s;
        s += n.child_mask.count_ones();
    }
    nodes
}

pub fn counts(nodes: &[Node]) -> TreeCounts {
    TreeCounts {
        node_count: nodes.len(),
        children_list_count: nodes
            .iter()
            .map(|n| n.child_mask.count_ones() as usize)
            .sum(),
        leaf_brick_count: nodes.iter().filter(|n| n.child_mask == 0).count(),
    }
}

/// Serialize into the `tree.bin` byte layout.
pub fn encode_tree_bin(nodes: &[Node]) -> Vec<u8> {
    let c = counts(nodes);
    let mut out = Vec::with_capacity(12 + 124 * nodes.len());

    // TREE: N × 48 B
    for (i, n) in nodes.iter().enumerate() {
        let header = if i == 0 {
            [
                c.node_count as u32,
                c.children_list_count as u32,
                c.leaf_brick_count as u32,
            ]
        } else {
            [0, 0, 0]
        };
        for v in header {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&0u32.to_le_bytes()); // t
        for v in [n.x, n.y, n.z, n.depth] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&n.child_mask.to_le_bytes());
        out.extend_from_slice(&n.start.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes()); // reserved
    }

    // MID: N × 72 B, zeros except (min, max) f32 at offset 16
    for n in nodes {
        let mut mid = [0u8; 72];
        mid[16..20].copy_from_slice(&(n.min as f32).to_le_bytes());
        mid[20..24].copy_from_slice(&(n.max as f32).to_le_bytes());
        out.extend_from_slice(&mid);
    }

    // TAIL: 16 zero bytes + flat children list
    out.extend_from_slice(&[0u8; 16]);
    for n in nodes {
        for &child in &n.children {
            out.extend_from_slice(&child.to_le_bytes());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_brick_tree() {
        let nodes = build_tree([1, 1, 1], &[(2.0, 5.0)], (0.0, 0.0));
        assert_eq!(nodes.len(), 1);
        assert_eq!(
            counts(&nodes),
            TreeCounts {
                node_count: 1,
                children_list_count: 0,
                leaf_brick_count: 1
            }
        );
        let bytes = encode_tree_bin(&nodes);
        assert_eq!(bytes.len(), 12 + 124);
        assert_eq!(u32::from_le_bytes(bytes[0..4].try_into().unwrap()), 1); // N
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1); // L
        assert_eq!(
            f32::from_le_bytes(bytes[48 + 16..48 + 20].try_into().unwrap()),
            2.0
        );
        assert_eq!(
            f32::from_le_bytes(bytes[48 + 20..48 + 24].try_into().unwrap()),
            5.0
        );
    }

    #[test]
    fn five_bricks_wide_has_two_levels() {
        // 5×1×1 bricks: root (depth 1) with children at x = 0..=3, then x = 4 needs another parent
        // slot... root covers 4 bricks per axis, so depth must be 2.
        assert_eq!(max_depth_for([5, 1, 1]), 2);
        let mm: Vec<_> = (0..5).map(|i| (f64::from(i), f64::from(i) + 0.5)).collect();
        let nodes = build_tree([5, 1, 1], &mm, (0.0, 0.0));
        // depth 2: 1 root; depth 1: 2 nodes (x=0,1); depth 0: 5 leaves
        assert_eq!(nodes.len(), 1 + 2 + 5);
        let c = counts(&nodes);
        assert_eq!(c.children_list_count, nodes.len() - 1);
        assert_eq!(c.leaf_brick_count, 5);
        assert_eq!(nodes[0].children, vec![1, 2]);
        assert_eq!(nodes[1].children, vec![3, 4, 5, 6]);
        assert_eq!(nodes[2].children, vec![7]);
        assert_eq!((nodes[0].min, nodes[0].max), (0.0, 4.5));
        assert_eq!(encode_tree_bin(&nodes).len(), 12 + 124 * nodes.len());
    }

    #[test]
    fn all_invalid_leaf_uses_fallback() {
        let nodes = build_tree(
            [1, 1, 1],
            &[(f64::INFINITY, f64::NEG_INFINITY)],
            (-1.0, 9.0),
        );
        assert_eq!((nodes[0].min, nodes[0].max), (-1.0, 9.0));
    }
}
