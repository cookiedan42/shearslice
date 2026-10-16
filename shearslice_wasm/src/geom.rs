use wasm_bindgen::prelude::*;

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
