#![allow(clippy::unwrap_used)]

use shearslice_core::{BinPath, BinStore, Error};
use shearslice_wasm::JsFnStore;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

fn store(body: &str) -> JsFnStore {
    JsFnStore::new(js_sys::Function::new_with_args("p", body))
}

#[wasm_bindgen_test]
async fn resolves_uint8array() {
    let s = store("return Promise.resolve(new Uint8Array([1, 2, 3]));");
    let bytes = s.get(&BinPath::from("a/b.bin")).await.unwrap();
    assert_eq!(&bytes[..], &[1, 2, 3]);
}

#[wasm_bindgen_test]
async fn resolves_arraybuffer() {
    let s = store("return Promise.resolve(new Uint8Array([9, 8]).buffer);");
    let bytes = s.get(&BinPath::from("a/b.bin")).await.unwrap();
    assert_eq!(&bytes[..], &[9, 8]);
}

#[wasm_bindgen_test]
async fn accepts_non_promise_return() {
    let s = store("return new Uint8Array([7]);");
    let bytes = s.get(&BinPath::from("a/b.bin")).await.unwrap();
    assert_eq!(&bytes[..], &[7]);
}

#[wasm_bindgen_test]
async fn passes_relative_path_to_js() {
    // echo the path back as bytes
    let s = store("return Promise.resolve(new TextEncoder().encode(p));");
    let bytes = s
        .get(&BinPath::from("variables/0/0/bundles/0/0-0-0.bin"))
        .await
        .unwrap();
    assert_eq!(&bytes[..], b"variables/0/0/bundles/0/0-0-0.bin");
}

#[wasm_bindgen_test]
async fn null_and_undefined_are_not_found() {
    for body in ["return Promise.resolve(null);", "return Promise.resolve(undefined);"] {
        let err = store(body).get(&BinPath::from("x.bin")).await.unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }), "{body}: {err:?}");
    }
}

#[wasm_bindgen_test]
async fn rejection_is_generic_with_message() {
    let s = store("return Promise.reject(new Error('boom'));");
    let err = s.get(&BinPath::from("x.bin")).await.unwrap_err();
    match err {
        Error::Generic { message, .. } => assert!(message.contains("boom"), "{message}"),
        Error::NotFound { .. } => panic!("expected Generic"),
    }
}

#[wasm_bindgen_test]
async fn synchronous_throw_is_generic() {
    let s = store("throw new Error('sync boom');");
    let err = s.get(&BinPath::from("x.bin")).await.unwrap_err();
    match err {
        Error::Generic { message, .. } => assert!(message.contains("sync boom"), "{message}"),
        Error::NotFound { .. } => panic!("expected Generic"),
    }
}
