use hashbrown::HashMap;
use js_sys::{Array, Uint8Array};
use shearslice_core::Layer;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    // Initialize the logger so that the logs are printed to the console
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
}
#[wasm_bindgen]
pub fn hello_world() -> String {
    "Hello, world!".to_string()
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

pub fn get_keys(layer: &Layer, layer_id: usize) -> Vec<usize> {
    let node_size = layer.index().node_size();

    let mut res = Vec::new();
    let paths = layer.tails(layer_id);

    for (file_x, file_y, file_z) in paths {
        res.push(file_x);
        res.push(file_y);
        res.push(file_z);
    }
    res
}

#[wasm_bindgen]
pub struct VoxelData {
    layer: shearslice_core::VoxelLayer,
    layer_id: usize,
}

#[wasm_bindgen]
impl VoxelData {
    pub fn get_corners(&self) -> Vec<f64> {
        let e = self.layer.layer().extent();

        vec![e.xmin(), e.xmax(), e.ymin(), e.ymax(), e.zmin(), e.zmax()]
    }

    pub fn make_layer(layer: &str, values: Array, layer_id: usize) -> Self {
        let layer = Layer::from_str(layer);

        let mut vec_values: Vec<Vec<u8>> = Vec::new();

        let node_size = layer.index().node_size();

        // Convert each item in the array to Vec<u8>
        for i in 0..values.length() {
            let item = values.get(i);
            let uint8_array = Uint8Array::from(item);
            vec_values.push(uint8_array.to_vec());
        }
        let mut map = HashMap::new();

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
            map.insert((layer_id, x, y, z), v);
        }

        Self {
            layer: shearslice_core::VoxelLayer::new(layer, map),
            layer_id,
        }
    }
}

#[wasm_bindgen]
impl VoxelData {
    pub fn query_plane(&self, z: usize) -> js_sys::Uint8Array {
        let data = self.layer.query_plane(z).unwrap();
        let [s, e] = self.layer.layer().styles().variable_styles()[0]
            .transfer_function()
            .stretch_range();

        // normalise data
        let res1: Vec<f32> = data
            .iter()
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
}
