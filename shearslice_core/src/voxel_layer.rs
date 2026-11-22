use crate::layer::Layer;
use crate::node::Node;
use crate::store::{BinStore, Error, InMemory, Path, Result};
use bytes::Bytes;
use core::cell::RefCell;
use hashbrown::{HashMap, HashSet};
use std::sync::Arc;

#[cfg(feature = "std")]
use crate::store::LocalFileSystem;

/// Node origin in brick-index units; matches the `X-Y-Z` in a bin's file name.
pub type NodeKey = (usize, usize, usize);

/// A voxel layer backed by any [`BinStore`].
///
/// Nodes are fetched lazily through the async methods and cached; the `*_cached`
/// methods are synchronous and only read nodes that are already loaded.
pub struct VoxelLayer {
    layer: Layer,
    store: Arc<dyn BinStore>,
    nodes: RefCell<HashMap<(usize, NodeKey), (Node<f32>, Bytes)>>,
    pub brick_size: [usize; 3],
    pub node_size: [usize; 3],
    pub apron_width: usize,
}

/// Constructors
impl VoxelLayer {
    #[must_use]
    pub fn new(layer: Layer, store: Arc<dyn BinStore>) -> Self {
        let node_size = layer.index().node_size();
        let brick_size = layer.index().brick_size();
        let apron_width = layer.index().apron_width();
        Self {
            layer,
            store,
            nodes: RefCell::new(HashMap::new()),
            brick_size,
            node_size,
            apron_width,
        }
    }

    /// Layer with an empty in-memory store; feed it with [`Self::insert_node`].
    #[must_use]
    pub fn from_layer_json(json: &str) -> Self {
        Self::new(Layer::from_str(json), Arc::new(InMemory::new()))
    }

    /// Open an unpacked eslpk directory.
    #[cfg(feature = "std")]
    #[must_use]
    pub fn from_file(path: &str) -> Self {
        let layer = Layer::from_file(&format!("{path}/layer.json"));
        Self::new(layer, Arc::new(LocalFileSystem::new(path)))
    }
}

/// Paths and geometry (pure, synchronous)
impl VoxelLayer {
    #[must_use]
    pub const fn layer(&self) -> &Layer {
        &self.layer
    }

    /// Path of the bin containing the grid index `(x, y, z)`.
    #[must_use]
    pub fn bin_path_for(&self, x: usize, y: usize, z: usize, layer_id: usize) -> Path {
        Path::bin(layer_id, self.node_key_for_indices(x, y, z))
    }

    /// All bin paths needed to cover plane `z`.
    #[must_use]
    pub fn bin_paths_for_plane(&self, z: usize, layer_id: usize) -> Vec<Path> {
        let dims = self.layer.volumes()[0].dimensions();
        let (x_len, y_len) = (dims[0].size(), dims[1].size());

        let mut paths = Vec::new();
        let mut seen = HashSet::new();
        for x in (0..x_len).step_by(self.brick_size[0] * self.node_size[0]) {
            for y in (0..y_len).step_by(self.brick_size[1] * self.node_size[1]) {
                let path = self.bin_path_for(x, y, z, layer_id);
                if seen.insert(path.clone()) {
                    paths.push(path);
                }
            }
        }
        paths
    }

    /// get all the x,y coordinates of the data
    /// used for elevation querying
    #[must_use]
    pub fn get_xy(&self) -> Vec<(f32, f32)> {
        let x_vals = self.layer().volumes()[0].dimensions()[0].values();
        let y_vals = self.layer().volumes()[0].dimensions()[1].values();

        x_vals
            .iter()
            .flat_map(|x| y_vals.iter().map(|y| (*x, *y)))
            .collect()
    }

    /// Convert a world/projected coordinate to a grid index.
    ///
    /// Vertical exaggeration is a rendering-only stretch applied when drawing the volume;
    /// it is not part of the stored grid geometry, so it is not applied here.
    #[must_use]
    pub fn indices_for_geometry(&self, x: f32, y: f32, z: f32) -> (usize, usize, usize) {
        let dims = self.layer().volumes()[0].dimensions();
        (
            dims[0].find(x, None),
            dims[1].find(y, None),
            dims[2].find(z, None),
        )
    }

    /// The node-origin key (in brick-index units) that a grid index falls in.
    /// This is the same key used by the node cache and `Path::bin`.
    #[must_use]
    pub fn node_key_for_indices(&self, x: usize, y: usize, z: usize) -> NodeKey {
        let brick_x = x / self.brick_size[0];
        let brick_y = y / self.brick_size[1];
        let brick_z = z / self.brick_size[2];

        (
            brick_x / self.node_size[0] * self.node_size[0],
            brick_y / self.node_size[1] * self.node_size[1],
            brick_z / self.node_size[2] * self.node_size[2],
        )
    }

    /// The node-origin key that a world/projected coordinate falls in.
    #[must_use]
    pub fn node_key_for_geometry(&self, x: f32, y: f32, z: f32) -> NodeKey {
        let (x_index, y_index, z_index) = self.indices_for_geometry(x, y, z);
        self.node_key_for_indices(x_index, y_index, z_index)
    }

    /// Whether a world/projected coordinate falls within the layer's full extent.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn in_extent(&self, x: f32, y: f32, z: f32) -> bool {
        let extent = self.layer().extent();
        // `extent` is parsed as f64 while coordinates and dimension spacing are f32
        // everywhere else in this API. Comparing in f64 by upcasting `x`/`y`/`z` can
        // spuriously reject a point sitting exactly on a boundary, because the extent
        // bound and the dimension's own edge value are independent roundings of the same
        // JSON literal into different precisions. Round the bounds down to f32 first so
        // the comparison matches the precision `find` actually operates in.
        let (xmin, xmax) = (extent.xmin() as f32, extent.xmax() as f32);
        let (ymin, ymax) = (extent.ymin() as f32, extent.ymax() as f32);
        let (zmin, zmax) = (extent.zmin() as f32, extent.zmax() as f32);
        x.is_finite()
            && y.is_finite()
            && z.is_finite()
            && x >= xmin
            && x <= xmax
            && y >= ymin
            && y <= ymax
            && z >= zmin
            && z <= zmax
    }

    fn in_grid(&self, x: usize, y: usize, z: usize) -> bool {
        let dims = self.layer().volumes()[0].dimensions();
        x < dims[0].size() && y < dims[1].size() && z < dims[2].size()
    }
}

/// Node cache (synchronous)
impl VoxelLayer {
    /// Parse and cache a node's bytes, replacing any existing entry.
    pub fn insert_node(&self, layer_id: usize, key: NodeKey, bytes: Bytes) {
        let node = Node::from_bin(
            &bytes,
            self.node_size,
            self.brick_size[0],
            self.brick_size[1],
            self.brick_size[2],
            self.apron_width,
        );
        self.nodes.borrow_mut().insert((layer_id, key), (node, bytes));
    }

    #[must_use]
    pub fn is_bin_loaded(&self, layer_id: usize, key: NodeKey) -> bool {
        self.nodes.borrow().contains_key(&(layer_id, key))
    }

    /// Drop a single cached node, freeing its memory.
    pub fn evict_bin(&self, layer_id: usize, key: NodeKey) {
        self.nodes.borrow_mut().remove(&(layer_id, key));
    }

    /// Drop every cached node.
    pub fn clear_bins(&self) {
        self.nodes.borrow_mut().clear();
    }
}

/// Cache-only queries (synchronous). A node that isn't loaded yields `Error::NotFound`.
impl VoxelLayer {
    /// Returns `Ok(None)` if `(x, y, z)` is outside the grid or holds the variable's
    /// no-data value.
    ///
    /// # Errors
    /// `Error::NotFound` if the containing node isn't loaded.
    pub fn query_cached(&self, x: usize, y: usize, z: usize, layer_id: usize) -> Result<Option<f32>> {
        if !self.in_grid(x, y, z) {
            return Ok(None);
        }
        let key = self.node_key_for_indices(x, y, z);
        let nodes = self.nodes.borrow();
        let (node, bytes) = nodes
            .get(&(layer_id, key))
            .ok_or_else(|| Error::NotFound {
                path: Path::bin(layer_id, key),
            })?;
        let raw = node.query(x, y, z, bytes);

        Ok(match self.layer.no_data_for(layer_id) {
            Some(no_data) if raw == no_data => None,
            _ => Some(raw),
        })
    }

    /// # Errors
    /// `Error::NotFound` if the containing node isn't loaded.
    pub fn query_geometry_cached(&self, x: f32, y: f32, z: f32, layer_id: usize) -> Result<Option<f32>> {
        if !self.in_extent(x, y, z) {
            return Ok(None);
        }
        let (xi, yi, zi) = self.indices_for_geometry(x, y, z);
        self.query_cached(xi, yi, zi, layer_id)
    }

    /// Values of plane `z` (clamped), x-major. `None` marks no-data voxels.
    ///
    /// # Errors
    /// `Error::NotFound` if a required node isn't loaded.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    pub fn query_plane_cached(&self, z: i32, layer_id: usize) -> Result<Vec<Option<f32>>> {
        let dims = self.layer().volumes()[0].dimensions();
        let (x_len, y_len, z_len) = (dims[0].size(), dims[1].size(), dims[2].size());
        let z = z.clamp(0, (z_len - 1) as i32) as usize;

        (0..x_len)
            .flat_map(|x| (0..y_len).map(move |y| (x, y)))
            .map(|(x, y)| self.query_cached(x, y, z, layer_id))
            .collect()
    }

    /// # Errors
    /// `Error::NotFound` if a required node isn't loaded.
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    pub fn query_arr_cached(&self, arr: &[usize], z: i32, layer_id: usize) -> Result<Vec<Option<f32>>> {
        let dims = self.layer().volumes()[0].dimensions();
        let (x_len, y_len, z_len) = (dims[0].size(), dims[1].size(), dims[2].size());

        arr.iter()
            .enumerate()
            .map(|(i, v)| {
                // x1 and y1 are lat lon so need to be flipped
                let x1 = i / x_len;
                let y1 = i % y_len;
                let z1 = (z + (*v as i32)).clamp(0, (z_len - 1) as i32) as usize;
                self.query_cached(x1, y1, z1, layer_id)
            })
            .collect()
    }
}

/// Async queries: fetch missing nodes from the store, then read from the cache.
impl VoxelLayer {
    /// Fetch a node from the store into the cache if it isn't there already.
    ///
    /// # Errors
    /// Any error from the underlying [`BinStore`].
    pub async fn load_node(&self, layer_id: usize, key: NodeKey) -> Result<()> {
        // The RefCell borrow must not live across the await below.
        if self.is_bin_loaded(layer_id, key) {
            return Ok(());
        }
        let bytes = self.store.get(&Path::bin(layer_id, key)).await?;
        self.insert_node(layer_id, key, bytes);
        Ok(())
    }

    /// # Errors
    /// Any error from the underlying [`BinStore`].
    pub async fn query(&self, x: usize, y: usize, z: usize, layer_id: usize) -> Result<Option<f32>> {
        if !self.in_grid(x, y, z) {
            return Ok(None);
        }
        self.load_node(layer_id, self.node_key_for_indices(x, y, z)).await?;
        self.query_cached(x, y, z, layer_id)
    }

    /// # Errors
    /// Any error from the underlying [`BinStore`].
    pub async fn query_geometry(&self, x: f32, y: f32, z: f32, layer_id: usize) -> Result<Option<f32>> {
        if !self.in_extent(x, y, z) {
            return Ok(None);
        }
        let (xi, yi, zi) = self.indices_for_geometry(x, y, z);
        self.query(xi, yi, zi, layer_id).await
    }

    /// # Errors
    /// Any error from the underlying [`BinStore`].
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    pub async fn query_plane(&self, z: i32, layer_id: usize) -> Result<Vec<Option<f32>>> {
        let z_len = self.layer().volumes()[0].dimensions()[2].size();
        let z = z.clamp(0, (z_len - 1) as i32) as usize;
        self.load_plane(z, layer_id).await?;
        self.query_plane_cached(z as i32, layer_id)
    }

    /// # Errors
    /// Any error from the underlying [`BinStore`].
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap, clippy::cast_sign_loss)]
    pub async fn query_arr(&self, arr: &[usize], z: i32, layer_id: usize) -> Result<Vec<Option<f32>>> {
        let z_len = self.layer().volumes()[0].dimensions()[2].size();
        // each point may land on a different plane, so load every plane it can touch
        let (lo, hi) = arr.iter().fold((usize::MAX, 0), |(lo, hi), v| {
            let zz = (z + (*v as i32)).clamp(0, (z_len - 1) as i32) as usize;
            (lo.min(zz), hi.max(zz))
        });
        if lo <= hi {
            for zz in lo..=hi {
                self.load_plane(zz, layer_id).await?;
            }
        }
        self.query_arr_cached(arr, z, layer_id)
    }

    /// Load every node intersecting plane `z`.
    async fn load_plane(&self, z: usize, layer_id: usize) -> Result<()> {
        let dims = self.layer().volumes()[0].dimensions();
        let (x_len, y_len) = (dims[0].size(), dims[1].size());
        for x in (0..x_len).step_by(self.brick_size[0] * self.node_size[0]) {
            for y in (0..y_len).step_by(self.brick_size[1] * self.node_size[1]) {
                self.load_node(layer_id, self.node_key_for_indices(x, y, z)).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn test_from_file() {
        let _layer = VoxelLayer::from_file("../data/30");
    }

    #[test]
    fn test_node_key_consistency_and_eviction() {
        let layer = VoxelLayer::from_file("../data/256");
        let dims = layer.layer().volumes()[0].dimensions();
        let x = dims[0].values()[100];
        let y = dims[1].values()[100];
        let z = dims[2].values()[20];
        let layer_id = 0;

        // plan and query must agree on which node a point lives in
        let planned_key = layer.node_key_for_geometry(x, y, z);
        let (ix, iy, iz) = layer.indices_for_geometry(x, y, z);
        assert_eq!(planned_key, layer.node_key_for_indices(ix, iy, iz));

        assert!(!layer.is_bin_loaded(layer_id, planned_key));
        // cache-only query on an unloaded node reports NotFound
        assert!(matches!(
            layer.query_geometry_cached(x, y, z, layer_id),
            Err(Error::NotFound { .. })
        ));
        let _ = block_on(layer.query_geometry(x, y, z, layer_id)).unwrap();
        assert!(layer.is_bin_loaded(layer_id, planned_key));

        layer.evict_bin(layer_id, planned_key);
        assert!(!layer.is_bin_loaded(layer_id, planned_key));

        let _ = block_on(layer.query_geometry(x, y, z, layer_id)).unwrap();
        layer.clear_bins();
        assert!(!layer.is_bin_loaded(layer_id, planned_key));
    }

    #[test]
    fn test_missing_bin_is_error_not_panic() {
        let layer = VoxelLayer::from_file("../data/256");
        let mem = VoxelLayer::new(
            Layer::from_file("../data/256/layer.json"),
            Arc::new(InMemory::new()),
        );
        assert!(layer.query_cached(0, 0, 0, 0).is_err());
        assert!(matches!(
            block_on(mem.query(0, 0, 0, 0)),
            Err(Error::NotFound { .. })
        ));
    }

    #[test]
    fn test_find_far_edge_no_panic() {
        let layer = VoxelLayer::from_file("../data/256");
        let dims = layer.layer().volumes()[0].dimensions();
        let x_vals = dims[0].values();
        let len = x_vals.len();

        // strictly inside the last interval of an irregular (binary-searched) dimension
        let x = (x_vals[len - 2] + x_vals[len - 1]) / 2.0;
        let y = dims[1].values()[0];
        let z = dims[2].values()[0];

        let _ = block_on(layer.query_geometry(x, y, z, 0));
    }

    #[test]
    fn test_bracket_rounds_to_nearest_index() {
        let layer = VoxelLayer::from_file("../data/256");
        let dims = layer.layer().volumes()[0].dimensions();
        let x_vals = dims[0].values(); // irregular spacing
        let z_vals = dims[2].values(); // regular spacing
        let y = dims[1].values()[0];

        let ix = 50;
        let x_lo = x_vals[ix];
        let x_hi = x_vals[ix + 1];
        let (found_near_lo, _, _) =
            layer.indices_for_geometry(x_lo + (x_hi - x_lo) * 0.1, y, z_vals[0]);
        let (found_near_hi, _, _) =
            layer.indices_for_geometry(x_lo + (x_hi - x_lo) * 0.9, y, z_vals[0]);
        assert_eq!(found_near_lo, ix);
        assert_eq!(found_near_hi, ix + 1);

        let iz = 10;
        let z_lo = z_vals[iz];
        let z_hi = z_vals[iz + 1];
        let (_, _, found_z_near_lo) =
            layer.indices_for_geometry(x_vals[0], y, z_lo + (z_hi - z_lo) * 0.1);
        let (_, _, found_z_near_hi) =
            layer.indices_for_geometry(x_vals[0], y, z_lo + (z_hi - z_lo) * 0.9);
        assert_eq!(found_z_near_lo, iz);
        assert_eq!(found_z_near_hi, iz + 1);
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)] // test-only: values are well within f32 range
    fn test_in_extent() {
        let layer = VoxelLayer::from_file("../data/256");
        let extent = layer.layer().extent();

        let mid_x = ((extent.xmin() + extent.xmax()) / 2.0) as f32;
        let mid_y = ((extent.ymin() + extent.ymax()) / 2.0) as f32;
        let mid_z = ((extent.zmin() + extent.zmax()) / 2.0) as f32;
        assert!(layer.in_extent(mid_x, mid_y, mid_z));

        assert!(!layer.in_extent((extent.xmax() + 1000.0) as f32, mid_y, mid_z));
        assert!(!layer.in_extent(mid_x, mid_y, (extent.zmin() - 1000.0) as f32));
        assert!(!layer.in_extent(f32::NAN, mid_y, mid_z));
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)] // test-only: values are well within f32 range
    fn test_query_geometry_out_of_bounds_returns_none() {
        let layer = VoxelLayer::from_file("../data/256");
        let dims = layer.layer().volumes()[0].dimensions();
        let (x, y, z) = (
            dims[0].values()[0],
            dims[1].values()[0],
            dims[2].values()[0],
        );

        // sanity: an in-bounds point returns Some
        assert!(block_on(layer.query_geometry(x, y, z, 0)).unwrap().is_some());

        let extent = layer.layer().extent();
        assert_eq!(
            block_on(layer.query_geometry((extent.xmax() + 1000.0) as f32, y, z, 0)).unwrap(),
            None
        );
        assert_eq!(block_on(layer.query_geometry(f32::NAN, y, z, 0)).unwrap(), None);
    }

    #[test]
    fn test_query_index_out_of_bounds_returns_none_without_panic() {
        let layer = VoxelLayer::from_file("../data/256");
        let dims = layer.layer().volumes()[0].dimensions();
        let (x_len, y_len, z_len) = (dims[0].size(), dims[1].size(), dims[2].size());

        // sanity: the last valid index still returns Some
        assert!(block_on(layer.query(x_len - 1, y_len - 1, z_len - 1, 0)).unwrap().is_some());

        // one past the end on each axis individually must not panic and must return None
        assert_eq!(block_on(layer.query(x_len, 0, 0, 0)).unwrap(), None);
        assert_eq!(block_on(layer.query(0, y_len, 0, 0)).unwrap(), None);
        assert_eq!(block_on(layer.query(0, 0, z_len, 0)).unwrap(), None);
    }
}
