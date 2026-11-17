use crate::store::{IdbCache, JsFnStore};
use shearslice_core::{Layer, VoxelLayer};
use std::rc::Rc;
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

/// Voxel queries backed by an `IndexedDB` read-through cache over a JS fetch function.
#[wasm_bindgen]
pub struct CachedVoxelQuery {
    layer: Rc<VoxelLayer>,
    layer_id: usize,
}

#[wasm_bindgen]
impl CachedVoxelQuery {
    /// `fetch` is `(path: string) => Promise<ArrayBuffer | Uint8Array | null>`, called with
    /// the bin's path relative to the layer root (JS adds the base URL and any auth).
    /// `cache_namespace` keys the cache entries; include a layer URL and version so a
    /// republished layer doesn't serve stale bins.
    ///
    /// # Errors
    /// Rejects if `IndexedDB` can't be opened.
    #[allow(clippy::future_not_send)] // wasm is single-threaded
    pub async fn open(
        layer_json: String,
        layer_id: usize,
        cache_namespace: String,
        fetch: js_sys::Function,
    ) -> Result<Self, JsValue> {
        let store = IdbCache::open("shearslice", cache_namespace, JsFnStore::new(fetch))
            .await
            .map_err(|e| JsValue::from_str(&e))?;
        let layer = VoxelLayer::new(Layer::from_str(&layer_json), Arc::new(store));
        Ok(Self {
            layer: Rc::new(layer),
            layer_id,
        })
    }

    /// Resolves to a `Float32Array` with one value per point; `NaN` means out of extent or
    /// no-data. Rejects if any needed bin can't be fetched.
    pub fn query_geometry(&self, xs: Vec<f32>, ys: Vec<f32>, zs: Vec<f32>) -> js_sys::Promise {
        let layer = Rc::clone(&self.layer);
        let layer_id = self.layer_id;
        future_to_promise(async move {
            let mut out = Vec::with_capacity(xs.len());
            for ((x, y), z) in xs.into_iter().zip(ys).zip(zs) {
                let v = layer
                    .query_geometry(x, y, z, layer_id)
                    .await
                    .map_err(|e| JsValue::from_str(&e.to_string()))?;
                out.push(v.unwrap_or(f32::NAN));
            }
            Ok(js_sys::Float32Array::from(out.as_slice()).into())
        })
    }
}
