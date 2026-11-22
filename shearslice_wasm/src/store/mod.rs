//! Browser-side [`shearslice_core::BinStore`] implementations.

mod idb;
mod js_fn;

pub use idb::IdbCache;
pub use js_fn::JsFnStore;
