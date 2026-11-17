use async_trait::async_trait;
use bytes::Bytes;
use indexed_db_futures::database::Database;
use indexed_db_futures::prelude::*;
use indexed_db_futures::transaction::TransactionMode;
use indexed_db_futures::typed_array::{Uint8Array, Uint8ArraySlice};
use shearslice_core::{BinPath, BinStore, Result};

const STORE: &str = "bins";

/// Read-through `IndexedDB` cache in front of another [`BinStore`].
///
/// A hit is served from `IndexedDB`; a miss is fetched from `inner`, written back, and
/// returned. Cache failures are logged and never fail a read.
pub struct IdbCache<S> {
    db: Database,
    /// Key prefix, so several layers can share one database.
    namespace: String,
    inner: S,
}

impl<S> core::fmt::Debug for IdbCache<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IdbCache")
            .field("namespace", &self.namespace)
            .finish_non_exhaustive()
    }
}

impl<S: BinStore> IdbCache<S> {
    /// Open (creating if needed) the `IndexedDB` database `db_name`.
    ///
    /// # Errors
    /// The `IndexedDB` error message if the database can't be opened.
    #[allow(clippy::future_not_send)] // wasm is single-threaded
    pub async fn open(
        db_name: &str,
        namespace: impl Into<String>,
        inner: S,
    ) -> core::result::Result<Self, String> {
        let db = Database::open(db_name)
            .with_version(1u8)
            .with_on_upgrade_needed(|_event, db| {
                if !db.object_store_names().any(|name| name == STORE) {
                    db.create_object_store(STORE).build()?;
                }
                Ok(())
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(Self {
            db,
            namespace: namespace.into(),
            inner,
        })
    }

    fn key(&self, path: &BinPath) -> String {
        format!("{}/{}", self.namespace, path.as_str())
    }

    #[allow(clippy::future_not_send)]
    async fn read(&self, key: &str) -> indexed_db_futures::OpenDbResult<Option<Bytes>> {
        let tx = self.db.transaction(STORE).build()?;
        let store = tx.object_store(STORE)?;
        let value: Option<Uint8Array> = store.get(key).primitive()?.await?;
        Ok(value.map(|v| Bytes::from(Vec::<u8>::from(v))))
    }

    #[allow(clippy::future_not_send)]
    async fn write(&self, key: &str, bytes: &[u8]) -> indexed_db_futures::OpenDbResult<()> {
        let tx = self
            .db
            .transaction(STORE)
            .with_mode(TransactionMode::Readwrite)
            .build()?;
        let store = tx.object_store(STORE)?;
        store
            .put(Uint8ArraySlice::new(bytes))
            .with_key(key)
            .primitive()?
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

#[async_trait(?Send)]
impl<S: BinStore> BinStore for IdbCache<S> {
    async fn get(&self, path: &BinPath) -> Result<Bytes> {
        let key = self.key(path);

        match self.read(&key).await {
            Ok(Some(bytes)) => return Ok(bytes),
            Ok(None) => {}
            Err(e) => log::warn!("idb read failed for {key}: {e}"),
        }

        // No transaction is held here: IDB transactions auto-commit once the event loop
        // is yielded without a pending request, so they can't span this await.
        let bytes = self.inner.get(path).await?;

        if let Err(e) = self.write(&key, &bytes).await {
            log::warn!("idb write failed for {key}: {e}");
        }
        Ok(bytes)
    }
}
