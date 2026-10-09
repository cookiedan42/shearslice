use crate::geom::{PointXy, PointXyz};
use crate::get_keys;
use js_sys::{Array, Uint8Array};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct VoxelData {
    layer: shearslice_core::VoxelLayer<shearslice_core::MemoryLoader>,
    layer_id: usize,
}

#[wasm_bindgen]
impl VoxelData {
    /// Get the extent of the layer in projected coordinates
    pub fn get_corners(&self) -> Vec<f64> {
        let e = self.layer.layer().extent();
        vec![e.xmin(), e.xmax(), e.ymin(), e.ymax(), e.zmin(), e.zmax()]
    }

    /// Get all the x,y coordinates of the data
    pub fn get_xy(&self) -> Vec<PointXy> {
        self.layer
            .get_xy()
            .into_iter()
            .map(|(x, y)| PointXy { x: x, y: y })
            .collect()
    }

    /// Create a new VoxelData from a layer json and a typed array of values
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
    /// `NaN` (test with `Number.isNaN` on the JS side) if the coordinate is out of
    /// bounds or that voxel holds the variable's declared no-data value.
    pub fn query_geometry(&self, x: f32, y: f32, z: f32, layer_id: usize) -> f32 {
        self.layer
            .query_geometry(x, y, z, layer_id)
            .unwrap_or(f32::NAN)
    }
}
