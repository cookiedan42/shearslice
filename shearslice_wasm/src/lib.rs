use shearslice_core::{Layer, VoxelLayer};
use wasm_bindgen::prelude::*;
mod geom;
mod voxeldata;
pub use voxeldata::VoxelData;
mod bundle;

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

pub(crate) fn get_keys<T: shearslice_core::BinLoader>(
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
