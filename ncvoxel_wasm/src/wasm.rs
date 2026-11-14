//! `wasm-bindgen` surface.

use js_sys::{Map, Uint8Array};
use wasm_bindgen::prelude::*;

use crate::Options;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Conversion options. All fields are optional; construct with `new ConvertOptions()`.
#[wasm_bindgen]
#[derive(Default)]
pub struct ConvertOptions {
    inner: Options,
}

#[wasm_bindgen]
impl ConvertOptions {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Layer name (default `"voxel"`).
    pub fn set_name(&mut self, name: &str) {
        self.inner.name = name.to_string();
    }

    /// Metres added to every x (`lon`) coordinate.
    pub fn set_shift_x(&mut self, v: f64) {
        self.inner.shift_x = v;
    }

    /// Metres added to every y (`lat`) coordinate.
    pub fn set_shift_y(&mut self, v: f64) {
        self.inner.shift_y = v;
    }

    /// Metres added to every z coordinate.
    pub fn set_shift_z(&mut self, v: f64) {
        self.inner.shift_z = v;
    }

    /// Override the spatial-reference WKID (e.g. 3414).
    pub fn set_wkid(&mut self, wkid: u32) {
        self.inner.wkid = Some(wkid);
    }

    /// Initial z multiplier; 1.0 (default) is true aspect.
    pub fn set_vertical_exaggeration(&mut self, v: f64) {
        self.inner.vertical_exaggeration = v;
    }
}

/// Convert a NetCDF file to `.slpk` bytes.
#[wasm_bindgen]
pub fn netcdf_to_slpk(nc: &[u8], opts: &ConvertOptions) -> Result<Uint8Array, JsError> {
    Ok(Uint8Array::from(
        crate::convert_to_slpk(nc, &opts.inner)?.as_slice(),
    ))
}

/// Convert a NetCDF file to an eslpk file map: `Map<string path, Uint8Array>` of uncompressed
/// files, e.g. `layer.json`, `variables/0/0/bundles/0/0-0-0.bin`.
#[wasm_bindgen]
pub fn netcdf_to_files(nc: &[u8], opts: &ConvertOptions) -> Result<Map, JsError> {
    let map = Map::new();
    crate::convert_with(nc, &opts.inner, &mut |path, data| {
        map.set(
            &JsValue::from_str(&path),
            &Uint8Array::from(data.as_slice()),
        );
        Ok(())
    })?;
    Ok(map)
}

/// Describe a NetCDF file: `{ dimensions, variables: [{ name, dimensions, dtype, unit, isVoxelData }],
/// spatialReference }`.
#[wasm_bindgen]
pub fn inspect_netcdf(nc: &[u8]) -> Result<JsValue, JsError> {
    let v = crate::inspect(nc)?;
    // Plain objects (not `Map`s) so the result is `JSON.stringify`-able.
    let ser = serde_wasm_bindgen::Serializer::json_compatible();
    serde::Serialize::serialize(&v, &ser).map_err(|e| JsError::new(&e.to_string()))
}
