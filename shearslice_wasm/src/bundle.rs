use hashbrown::HashMap;
use shearslice_core::{MemoryLoader, VoxelLayer};
use wasm_bindgen::prelude::*;

/// Sentinel used in `BundlePlan::assignments` for a point that has no data:
/// outside the layer's extent, or a non-finite coordinate.
const NO_DATA: u32 = u32::MAX;

/// A plan for fetching only the node bundles a batch of points actually touches.
///
/// `bundle_ids[b]` is a `"{node_x}-{node_y}-{node_z}"` node-origin key (brick-index units,
/// the same convention as the SceneServer bundle path suffix
/// `variables/{layer_id}/0/bundles/0/{node_x}-{node_y}-{node_z}`, without the `.bin`
/// extension). `assignments[i]` is the index into `bundle_ids` for point `i`, or
/// [`NO_DATA`] if the point falls outside the layer's extent or has a non-finite coordinate.
#[wasm_bindgen]
pub struct BundlePlan {
    bundle_ids: Vec<String>,
    assignments: Vec<u32>,
}

#[wasm_bindgen]
impl BundlePlan {
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn bundle_ids(&self) -> Vec<String> {
        self.bundle_ids.clone()
    }

    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn assignments(&self) -> Vec<u32> {
        self.assignments.clone()
    }
}

/// Memory-lean point query over a voxel layer: plan which node bundles a batch of points
/// touches, then query each node's points and evict it, so only the nodes actually in
/// flight are ever resident in memory.
#[wasm_bindgen]
pub struct VoxelQuery {
    layer: VoxelLayer<MemoryLoader>,
    layer_id: usize,
}

#[wasm_bindgen]
impl VoxelQuery {
    /// `layer_json` is the layer's `layer.json`/SceneServer layer metadata text.
    /// `layer_id` selects the variable to query.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(layer_json: &str, layer_id: usize) -> Self {
        let layer = VoxelLayer::<MemoryLoader>::from_layer_json(layer_json);
        Self { layer, layer_id }
    }

    /// Group points (in the layer's projected spatial reference) into the minimal set of
    /// node bundles that need to be fetched to answer them.
    pub fn plan(&self, xs: &[f32], ys: &[f32], zs: &[f32]) -> BundlePlan {
        let mut bundle_ids = Vec::new();
        let mut node_to_bundle: HashMap<(usize, usize, usize), u32> = HashMap::new();
        let mut assignments = Vec::with_capacity(xs.len());

        for i in 0..xs.len() {
            let (x, y, z) = (xs[i], ys[i], zs[i]);

            if !self.layer.in_extent(x, y, z) {
                assignments.push(NO_DATA);
                continue;
            }

            let node_key = self.layer.node_key_for_geometry(x, y, z);
            let bundle_index = *node_to_bundle.entry(node_key).or_insert_with(|| {
                let (nx, ny, nz) = node_key;
                bundle_ids.push(format!("{nx}-{ny}-{nz}"));
                u32::try_from(bundle_ids.len() - 1).unwrap_or(NO_DATA)
            });
            assignments.push(bundle_index);
        }

        BundlePlan {
            bundle_ids,
            assignments,
        }
    }

    /// Query the raw f32 value at each of `xs`/`ys`/`zs`, which must be exactly the points
    /// planned into the node `(node_x, node_y, node_z)` (i.e. one bundle's worth from
    /// `plan`). `bytes` is that bundle's fetched contents. The node is preloaded, queried,
    /// then evicted before returning, so it does not stay resident afterwards.
    /// A point whose voxel holds the variable's declared no-data value is returned as
    /// `f32::NAN` (test with `Number.isNaN` on the JS side), consistent with [`NO_DATA`]
    /// marking an out-of-extent point in `plan`.
    pub fn query_bundle(
        &self,
        node_x: usize,
        node_y: usize,
        node_z: usize,
        bytes: Vec<u8>,
        xs: &[f32],
        ys: &[f32],
        zs: &[f32],
    ) -> Vec<f32> {
        self.layer
            .preload_bin(self.layer_id, node_x, node_y, node_z, bytes);

        let values = (0..xs.len())
            .map(|i| {
                self.layer
                    .query_geometry(xs[i], ys[i], zs[i], self.layer_id)
                    .unwrap_or(f32::NAN)
            })
            .collect();

        self.layer.evict_bin(self.layer_id, node_x, node_y, node_z);

        values
    }
}
