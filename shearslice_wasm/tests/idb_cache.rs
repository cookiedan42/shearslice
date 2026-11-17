#![allow(clippy::unwrap_used)]

use async_trait::async_trait;
use bytes::Bytes;
use shearslice_core::{BinPath, BinStore, Error, Result};
use shearslice_wasm::IdbCache;
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

wasm_bindgen_test_configure!(run_in_browser);

const DB: &str = "shearslice_test";

/// Inner store that counts calls and serves one fixed object at `known.bin`.
#[derive(Debug, Clone)]
struct Counting {
    calls: Rc<Cell<usize>>,
    payload: Bytes,
}

impl Counting {
    fn new(payload: &'static [u8]) -> Self {
        Self {
            calls: Rc::new(Cell::new(0)),
            payload: Bytes::from_static(payload),
        }
    }
}

#[async_trait(?Send)]
impl BinStore for Counting {
    async fn get(&self, path: &BinPath) -> Result<Bytes> {
        self.calls.set(self.calls.get() + 1);
        if path.as_str() == "known.bin" {
            Ok(self.payload.clone())
        } else {
            Err(Error::NotFound { path: path.clone() })
        }
    }
}

/// IndexedDB persists across tests in one browser session, so isolate by namespace.
fn unique_ns(tag: &str) -> String {
    format!("{tag}-{}", js_sys::Math::random())
}

#[wasm_bindgen_test]
async fn second_read_is_served_from_cache() {
    let inner = Counting::new(b"hello");
    let cache = IdbCache::open(DB, unique_ns("hit"), inner.clone()).await.unwrap();
    let path = BinPath::from("known.bin");

    assert_eq!(&cache.get(&path).await.unwrap()[..], b"hello");
    assert_eq!(inner.calls.get(), 1);

    assert_eq!(&cache.get(&path).await.unwrap()[..], b"hello");
    assert_eq!(inner.calls.get(), 1, "second read must not reach the inner store");
}

#[wasm_bindgen_test]
async fn cache_persists_across_instances() {
    let ns = unique_ns("persist");
    let path = BinPath::from("known.bin");

    let first = Counting::new(b"persisted");
    let c1 = IdbCache::open(DB, ns.clone(), first.clone()).await.unwrap();
    c1.get(&path).await.unwrap();
    assert_eq!(first.calls.get(), 1);
    drop(c1);

    // A fresh instance (as after a page reload) must hit IndexedDB, not the inner store.
    let second = Counting::new(b"persisted");
    let c2 = IdbCache::open(DB, ns, second.clone()).await.unwrap();
    assert_eq!(&c2.get(&path).await.unwrap()[..], b"persisted");
    assert_eq!(second.calls.get(), 0);
}

#[wasm_bindgen_test]
async fn namespaces_are_isolated() {
    let path = BinPath::from("known.bin");

    let a = Counting::new(b"a");
    IdbCache::open(DB, unique_ns("iso-a"), a.clone())
        .await
        .unwrap()
        .get(&path)
        .await
        .unwrap();

    // different namespace, same path: must miss and go to its own inner store
    let b = Counting::new(b"b");
    let cb = IdbCache::open(DB, unique_ns("iso-b"), b.clone()).await.unwrap();
    assert_eq!(&cb.get(&path).await.unwrap()[..], b"b");
    assert_eq!(b.calls.get(), 1);
}

#[wasm_bindgen_test]
async fn inner_not_found_propagates_and_is_not_cached() {
    let inner = Counting::new(b"x");
    let cache = IdbCache::open(DB, unique_ns("nf"), inner.clone()).await.unwrap();
    let path = BinPath::from("missing.bin");

    for expected_calls in 1..=2 {
        let err = cache.get(&path).await.unwrap_err();
        assert!(matches!(err, Error::NotFound { .. }), "{err:?}");
        assert_eq!(inner.calls.get(), expected_calls, "misses must not be cached");
    }
}

#[wasm_bindgen_test]
async fn binary_payload_round_trips_exactly() {
    static DATA: [u8; 256] = {
        let mut a = [0u8; 256];
        let mut i = 0;
        while i < 256 {
            a[i] = i as u8;
            i += 1;
        }
        a
    };
    let inner = Counting::new(&DATA);
    let ns = unique_ns("bin");
    let path = BinPath::from("known.bin");

    IdbCache::open(DB, ns.clone(), inner).await.unwrap().get(&path).await.unwrap();

    // read back purely from IndexedDB
    let cold = Counting::new(b"unused");
    let from_idb = IdbCache::open(DB, ns, cold.clone()).await.unwrap().get(&path).await.unwrap();
    assert_eq!(cold.calls.get(), 0);
    assert_eq!(&from_idb[..], &DATA[..]);
}
