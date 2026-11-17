use async_trait::async_trait;
use bytes::Bytes;
use shearslice_core::{BinPath, BinStore, Error, Result};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

/// A [`BinStore`] that delegates fetching to a JS function, so the caller can use
/// whatever authenticated transport it needs (e.g. the `ArcGIS` SDK's `esriRequest`).
///
/// The function receives the bin's relative path (e.g. `variables/0/0/bundles/0/0-0-0.bin`)
/// and must return (a promise of) an `ArrayBuffer` or `Uint8Array`. Resolving to
/// `null`/`undefined` means "no such bin".
pub struct JsFnStore {
    fetch: js_sys::Function,
}

impl JsFnStore {
    #[must_use]
    pub const fn new(fetch: js_sys::Function) -> Self {
        Self { fetch }
    }
}

impl core::fmt::Debug for JsFnStore {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("JsFnStore").finish_non_exhaustive()
    }
}

fn js_message(e: &JsValue) -> String {
    e.dyn_ref::<js_sys::Error>()
        .map_or_else(|| format!("{e:?}"), |err| String::from(err.message()))
}

#[async_trait(?Send)]
impl BinStore for JsFnStore {
    async fn get(&self, path: &BinPath) -> Result<Bytes> {
        let generic = |e: &JsValue| Error::Generic {
            path: path.clone(),
            message: js_message(e),
        };

        let ret = self
            .fetch
            .call1(&JsValue::NULL, &JsValue::from_str(path.as_str()))
            .map_err(|e| generic(&e))?;
        // `Promise::resolve` also accepts a plain (non-promise) return value.
        let value = JsFuture::from(js_sys::Promise::resolve(&ret))
            .await
            .map_err(|e| generic(&e))?;

        if value.is_null() || value.is_undefined() {
            return Err(Error::NotFound { path: path.clone() });
        }
        Ok(Bytes::from(js_sys::Uint8Array::new(&value).to_vec()))
    }
}
