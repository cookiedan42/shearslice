#![allow(clippy::unwrap_used)]

use bytes::Bytes;
use shearslice_core::{Layer, VoxelLayer};
use shearslice_wasm::CachedVoxelQuery;
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

const LAYER_JSON: &str = include_str!("../../data/30/layer.json");
const BIN: &[u8] = include_bytes!("../../data/30/variables/0/0/bundles/0/0-0-0.bin");
const BIN_PATH: &str = "variables/0/0/bundles/0/0-0-0.bin";

/// JS fetch function serving `BIN` at `BIN_PATH`, `null` elsewhere; counts invocations.
fn fetch_fn(calls: &Rc<Cell<usize>>) -> js_sys::Function {
    let calls = Rc::clone(calls);
    let closure = Closure::<dyn Fn(String) -> JsValue>::new(move |path: String| {
        calls.set(calls.get() + 1);
        let value = if path == BIN_PATH {
            js_sys::Uint8Array::from(BIN).into()
        } else {
            JsValue::NULL
        };
        js_sys::Promise::resolve(&value).into()
    });
    let f: js_sys::Function = closure.as_ref().clone().unchecked_into();
    closure.forget(); // test-only: keep the callback alive for the whole run
    f
}

fn unique_ns(tag: &str) -> String {
    format!("{tag}-{}", js_sys::Math::random())
}

/// A handful of in-extent points taken from the layer's own dimension values.
fn sample_points() -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let layer = Layer::from_str(LAYER_JSON);
    let dims = layer.volumes()[0].dimensions();
    let (xv, yv, zv) = (dims[0].values(), dims[1].values(), dims[2].values());
    let idx = |len: usize| [0, len / 3, len / 2, len - 1];
    let mut pts = (Vec::new(), Vec::new(), Vec::new());
    for i in 0..4 {
        pts.0.push(xv[idx(xv.len())[i]]);
        pts.1.push(yv[idx(yv.len())[i]]);
        pts.2.push(zv[idx(zv.len())[i]]);
    }
    pts
}

/// Ground truth from core directly, bypassing the cache and the JS callback.
fn expected(xs: &[f32], ys: &[f32], zs: &[f32]) -> Vec<f32> {
    let layer = VoxelLayer::from_layer_json(LAYER_JSON);
    layer.insert_node(0, (0, 0, 0), Bytes::from_static(BIN));
    (0..xs.len())
        .map(|i| {
            layer
                .query_geometry_cached(xs[i], ys[i], zs[i], 0)
                .unwrap()
                .unwrap_or(f32::NAN)
        })
        .collect()
}

async fn query(q: &CachedVoxelQuery, xs: &[f32], ys: &[f32], zs: &[f32]) -> Result<Vec<f32>, JsValue> {
    let v = JsFuture::from(q.query_geometry(xs.to_vec(), ys.to_vec(), zs.to_vec())).await?;
    Ok(js_sys::Float32Array::new(&v).to_vec())
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits() || x == y)
}

#[wasm_bindgen_test]
async fn matches_core_and_fetches_each_bin_once() {
    let calls = Rc::new(Cell::new(0));
    let q = CachedVoxelQuery::open(LAYER_JSON.into(), 0, unique_ns("q"), fetch_fn(&calls))
        .await
        .unwrap();
    let (xs, ys, zs) = sample_points();
    let want = expected(&xs, &ys, &zs);

    let got = query(&q, &xs, &ys, &zs).await.unwrap();
    assert!(same(&got, &want), "got {got:?}, want {want:?}");
    assert_eq!(calls.get(), 1);

    // in-memory node cache: no further fetches
    let again = query(&q, &xs, &ys, &zs).await.unwrap();
    assert!(same(&again, &want));
    assert_eq!(calls.get(), 1);
}

#[wasm_bindgen_test]
async fn reopen_with_same_namespace_uses_indexeddb() {
    let ns = unique_ns("reopen");
    let (xs, ys, zs) = sample_points();
    let want = expected(&xs, &ys, &zs);

    let calls1 = Rc::new(Cell::new(0));
    let q1 = CachedVoxelQuery::open(LAYER_JSON.into(), 0, ns.clone(), fetch_fn(&calls1))
        .await
        .unwrap();
    query(&q1, &xs, &ys, &zs).await.unwrap();
    assert_eq!(calls1.get(), 1);
    drop(q1);

    // new instance = empty node cache, as after a page reload
    let calls2 = Rc::new(Cell::new(0));
    let q2 = CachedVoxelQuery::open(LAYER_JSON.into(), 0, ns, fetch_fn(&calls2))
        .await
        .unwrap();
    let got = query(&q2, &xs, &ys, &zs).await.unwrap();
    assert!(same(&got, &want));
    assert_eq!(calls2.get(), 0, "bin must come from IndexedDB, not the fetch callback");
}

#[wasm_bindgen_test]
async fn out_of_extent_point_is_nan_without_fetching() {
    let calls = Rc::new(Cell::new(0));
    let q = CachedVoxelQuery::open(LAYER_JSON.into(), 0, unique_ns("oob"), fetch_fn(&calls))
        .await
        .unwrap();
    let got = query(&q, &[f32::MAX], &[f32::MAX], &[f32::MAX]).await.unwrap();
    assert!(got[0].is_nan());
    assert_eq!(calls.get(), 0);
}

#[wasm_bindgen_test]
async fn missing_bin_rejects() {
    // callback that always reports "no such bin"
    let f = js_sys::Function::new_with_args("p", "return Promise.resolve(null);");
    let q = CachedVoxelQuery::open(LAYER_JSON.into(), 0, unique_ns("miss"), f)
        .await
        .unwrap();
    let (xs, ys, zs) = sample_points();
    let err = query(&q, &xs[..1], &ys[..1], &zs[..1]).await.unwrap_err();
    assert!(err.as_string().unwrap().contains("not found"), "{err:?}");
}

#[wasm_bindgen_test]
async fn fetch_failure_rejects_with_message() {
    let f = js_sys::Function::new_with_args("p", "return Promise.reject(new Error('auth expired'));");
    let q = CachedVoxelQuery::open(LAYER_JSON.into(), 0, unique_ns("fail"), f)
        .await
        .unwrap();
    let (xs, ys, zs) = sample_points();
    let err = query(&q, &xs[..1], &ys[..1], &zs[..1]).await.unwrap_err();
    assert!(err.as_string().unwrap().contains("auth expired"), "{err:?}");
}
