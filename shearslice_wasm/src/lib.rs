use hashbrown::HashMap;
use js_sys::{Array, Uint8Array};
use shearslice_core::{Layer, MemoryLoader, VoxelLayer};
use wasm_bindgen::prelude::*;

/// Sentinel used in `BundlePlan::assignments` for a point that has no data:
/// outside the layer's extent, or a non-finite coordinate.
const NO_DATA: u32 = u32::MAX;

#[wasm_bindgen(start)]
pub fn start() {
    // Initialize the logger so that the logs are printed to the console
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
}

// wasm bindgen to create the hashmap
// feed into the voxel layer
// use the voxel layer to query the data

/// url should look like:  "https://.../rest/services/f773/SceneServer"
/// responses will append  "/variables/0/0/bundles/0/0-0-0"
#[wasm_bindgen]
pub fn get_paths(url: &str, layer_json: &str, layer_id: usize) -> Vec<String> {
    let layer = Layer::from_str(layer_json);

    let node_size = layer.index().node_size();

    let mut res = Vec::new();
    let paths = layer.tails(layer_id);

    for (file_x, file_y, file_z) in paths {
        let bin_path = format!(
            "{}/variables/{}/0/bundles/0/{}-{}-{}",
            url,
            layer_id,
            file_x * node_size[0],
            file_y * node_size[1],
            file_z * node_size[2]
        );
        res.push(bin_path);
    }
    res
}

pub fn get_keys<T: shearslice_core::BinLoader>(
    layer: &VoxelLayer<T>,
    layer_id: usize,
) -> Vec<usize> {
    let mut res = Vec::new();
    let paths = layer.layer().tails(layer_id);

    for (file_x, file_y, file_z) in paths {
        res.push(file_x);
        res.push(file_y);
        res.push(file_z);
    }
    res
}

#[wasm_bindgen]
pub struct VoxelData {
    layer: shearslice_core::VoxelLayer<shearslice_core::MemoryLoader>,
    layer_id: usize,
}

#[wasm_bindgen]
pub struct PointXy {
    pub x: f32,
    pub y: f32,
}
#[wasm_bindgen]
impl PointXy {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[wasm_bindgen]
pub struct PointXyz {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
#[wasm_bindgen]
impl PointXyz {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

#[wasm_bindgen]
impl VoxelData {
    pub fn get_corners(&self) -> Vec<f64> {
        let e = self.layer.layer().extent();
        vec![e.xmin(), e.xmax(), e.ymin(), e.ymax(), e.zmin(), e.zmax()]
    }
    pub fn get_xy(&self) -> Vec<PointXy> {
        self.layer
            .get_xy()
            .into_iter()
            .map(|(x, y)| PointXy { x: x, y: y })
            .collect()
    }

    pub fn make_layer(layer: &str, values: Array, layer_id: usize) -> Self {
        let layer =
            shearslice_core::VoxelLayer::<shearslice_core::MemoryLoader>::from_layer_json(layer);

        let mut vec_values: Vec<Vec<u8>> = Vec::new();

        let node_size = layer.node_size;
        // Convert each item in the array to Vec<u8>
        for i in 0..values.length() {
            let item = values.get(i);
            let uint8_array = Uint8Array::from(item);
            vec_values.push(uint8_array.to_vec());
        }

        for (x, y, z, v) in get_keys(&layer, layer_id)
            .chunks(3)
            .zip(vec_values.into_iter())
            .map(|(k, v)| {
                let x = k[0] * node_size[0];
                let y = k[1] * node_size[1];
                let z = k[2] * node_size[2];
                return (x, y, z, v);
            })
        {
            layer.preload_bin(layer_id, x, y, z, v);
        }

        Self { layer, layer_id }
    }
}

#[wasm_bindgen]
impl VoxelData {
    /// Query the data at a given z index plane
    /// Clamps the value to the range of the z variable
    /// No-data voxels are rendered as `f32::NAN`, which `apply_color_ramp`'s clamp turns
    /// into the bottom of the color ramp.
    pub fn query_plane(&self, z: i32) -> js_sys::Uint8Array {
        let data = self.layer.query_plane(z).unwrap();
        let [s, e] = self.layer.layer().styles().variable_styles()[0]
            .transfer_function()
            .stretch_range();

        // normalise data
        let res1: Vec<f32> = data
            .iter()
            .map(|v| v.unwrap_or(f32::NAN))
            .map(|v| v.clamp(*s, *e))
            .map(|v| (v - *s) / (*e - *s))
            .collect();

        let img_data = shearslice_core::to_png(
            &res1,
            self.layer.layer().styles().variable_styles()[0]
                .transfer_function()
                .color_stops(),
        );

        js_sys::Uint8Array::from(&img_data[..])
    }

    /// Query the data at a given array of x,y,z indices
    /// arranged as a flattened array of 1024 * 1024 u16 values
    /// returns a png image
    /// No-data voxels are rendered as `f32::NAN`, which `apply_color_ramp`'s clamp turns
    /// into the bottom of the color ramp.
    pub fn query_arr16(&self, arr16: &[u16], z: i32) -> js_sys::Uint8Array {
        let data = self
            .layer
            .query_arr(
                &arr16.iter().map(|v| *v as usize).collect::<Vec<usize>>(),
                z,
            )
            .unwrap();
        let [s, e] = self.layer.layer().styles().variable_styles()[0]
            .transfer_function()
            .stretch_range();

        // normalise data
        let res1: Vec<f32> = data
            .iter()
            .map(|v| v.unwrap_or(f32::NAN))
            .map(|v| v.clamp(*s, *e))
            .map(|v| (v - *s) / (*e - *s))
            .collect();

        let img_data = shearslice_core::to_png(
            &res1,
            self.layer.layer().styles().variable_styles()[0]
                .transfer_function()
                .color_stops(),
        );
        js_sys::Uint8Array::from(&img_data[..])
    }

    /// Query the data at a given array of x,y,z coordinates
    /// return a png image of the data
    /// No-data voxels are rendered as `f32::NAN`, which `apply_color_ramp`'s clamp turns
    /// into the bottom of the color ramp.
    pub fn query_arr_geometry(&self, arr: Vec<PointXyz>, layer_id: usize) -> js_sys::Uint8Array {
        let data = arr
            .iter()
            .map(|v| self.layer.query_geometry(v.x, v.y, v.z, layer_id))
            .collect::<Vec<_>>();
        let [s, e] = self.layer.layer().styles().variable_styles()[0]
            .transfer_function()
            .stretch_range();

        // normalise data
        let res1: Vec<f32> = data
            .iter()
            .map(|v| v.unwrap_or(f32::NAN))
            .map(|v| v.clamp(*s, *e))
            .map(|v| (v - *s) / (*e - *s))
            .collect();

        let img_data = shearslice_core::to_png(
            &res1,
            self.layer.layer().styles().variable_styles()[0]
                .transfer_function()
                .color_stops(),
        );
        js_sys::Uint8Array::from(&img_data[..])
    }

    /// Query the data at a given x,y,z coordinate
    /// Returns the raw, unclamped value of the variable at the nearest voxel, or
    /// `undefined` if that voxel holds the variable's declared no-data value.
    pub fn query_geometry(&self, x: f32, y: f32, z: f32, layer_id: usize) -> Option<f32> {
        self.layer.query_geometry(x, y, z, layer_id)
    }
}

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
