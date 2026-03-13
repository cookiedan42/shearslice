use shearslice_core::Layer;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn hello_world() -> String {
    "Hello, world!".to_string()
}

// wasm bindgen to create the hashmap
// feed into the voxel layer
// use the voxel layer to query the data

/// url should look like:  "https://tiles.arcgis.com/tiles/7sGQ5Pc8sV6zAmxs/arcgis/rest/services/f773/SceneServer"
/// responses will append  "/variables/0/0/bundles/0/0-12-0"
#[wasm_bindgen]
pub fn get_paths(url: &str, layer_json: &str, layer_id: usize) -> Vec<String> {
    let layer = Layer::from_str(layer_json);

    let node_size = layer.index().node_size();

    let mut res = Vec::new();
    let paths = layer.tails(layer_id);

    for (file_x, file_y, file_z) in paths {
        let bin_path = format!(
            "{}/variables/{}/0/bundles/0/{}-{}-{}.bin",
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
