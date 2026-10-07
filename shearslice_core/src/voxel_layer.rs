use crate::layer::Layer;
use crate::node;
use core::cell::RefCell;
use hashbrown::HashMap;

/// Trait for loading binary data from different sources
pub trait BinLoader {
    fn load(&self, path: &str) -> Result<Vec<u8>, String>;
}

/// File system loader for native builds
#[cfg(feature = "std")]
pub struct FileLoader {
    base_path: String,
}

#[cfg(feature = "std")]
impl FileLoader {
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: base_path.to_string(),
        }
    }
}

#[cfg(feature = "std")]
impl BinLoader for FileLoader {
    fn load(&self, path: &str) -> Result<Vec<u8>, String> {
        std::fs::read(format!("{}/{}", self.base_path, path))
            .map_err(|e| format!("Failed to read {}: {}", path, e))
    }
}

/// In-memory loader for pre-loaded data (works in WASM)
pub struct MemoryLoader {
    data: HashMap<String, Vec<u8>>,
}

impl MemoryLoader {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn insert(&mut self, path: &str, data: Vec<u8>) {
        self.data.insert(path.to_string(), data);
    }
}

impl Default for MemoryLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl BinLoader for MemoryLoader {
    fn load(&self, path: &str) -> Result<Vec<u8>, String> {
        self.data
            .get(path)
            .cloned()
            .ok_or_else(|| format!("Data not found: {}", path))
    }
}

pub struct VoxelLayer<L: BinLoader> {
    layer: Layer,
    loader: L,
    bins: RefCell<HashMap<(usize, usize, usize, usize), node::Node<f32>>>,
    pub brick_size: [usize; 3],
    pub node_size: [usize; 3],
    pub apron_width: usize,
}

/// Constructors
impl<L: BinLoader> VoxelLayer<L> {
    /// Create a new VoxelLayer with a custom loader
    pub fn with_loader(layer: Layer, loader: L) -> Self {
        let node_size = layer.index().node_size();
        let brick_size = layer.index().brick_size();
        let apron_width = layer.index().apron_width();

        Self {
            layer,
            loader,
            bins: RefCell::new(HashMap::new()),
            brick_size,
            node_size,
            apron_width,
        }
    }

    /// The number of bricks actually stored in the node whose origin (in brick-index
    /// units) is `(node_x, node_y, node_z)`, in each dimension.
    ///
    /// This is normally the configured `node_size`, but a node on the far edge of the
    /// volume stores fewer bricks when the volume's brick grid doesn't divide evenly by
    /// `node_size` (e.g. format2's 191-wide x axis has ceil(191/32)=6 bricks, so the node
    /// at brick-offset 4 only stores 2 bricks in x, not a full 4).
    fn node_brick_extents(&self, node_x: usize, node_y: usize, node_z: usize) -> [usize; 3] {
        let dims = self.layer.volumes()[0].dimensions();
        let extent = |node_origin: usize, brick_size: usize, node_size: usize, dim_size: usize| {
            let total_bricks = dim_size.div_ceil(brick_size);
            total_bricks.saturating_sub(node_origin).min(node_size)
        };
        [
            extent(
                node_x,
                self.brick_size[0],
                self.node_size[0],
                dims[0].size(),
            ),
            extent(
                node_y,
                self.brick_size[1],
                self.node_size[1],
                dims[1].size(),
            ),
            extent(
                node_z,
                self.brick_size[2],
                self.node_size[2],
                dims[2].size(),
            ),
        ]
    }

    /// Load a specific bin on demand
    fn load_bin(&self, layer_id: usize, node_x: usize, node_y: usize, node_z: usize) {
        let key = (layer_id, node_x, node_y, node_z);

        if self.bins.borrow().contains_key(&key) {
            return;
        }

        let bin_path = format!(
            "variables/{}/0/bundles/0/{}-{}-{}.bin",
            layer_id, node_x, node_y, node_z
        );

        let contents = self
            .loader
            .load(&bin_path)
            .unwrap_or_else(|e| panic!("{}", e));

        let [bx, by, bz] = self.node_brick_extents(node_x, node_y, node_z);
        let node = node::Node::from_bin(
            &contents,
            bx,
            by,
            bz,
            self.brick_size[0],
            self.brick_size[1],
            self.brick_size[2],
            self.apron_width,
            node_x * self.brick_size[0],
            node_y * self.brick_size[1],
            node_z * self.brick_size[2],
        );

        self.bins.borrow_mut().insert(key, node);
    }

    /// Preload a specific bin
    pub fn preload_bin(
        &self,
        layer_id: usize,
        node_x: usize,
        node_y: usize,
        node_z: usize,
        data: Vec<u8>,
    ) {
        let key = (layer_id, node_x, node_y, node_z);

        let [bx, by, bz] = self.node_brick_extents(node_x, node_y, node_z);
        let node = node::Node::from_bin(
            &data,
            bx,
            by,
            bz,
            self.brick_size[0],
            self.brick_size[1],
            self.brick_size[2],
            self.apron_width,
            node_x * self.brick_size[0],
            node_y * self.brick_size[1],
            node_z * self.brick_size[2],
        );

        self.bins.borrow_mut().insert(key, node);
    }

    /// Check if a bin is loaded
    pub fn is_bin_loaded(
        &self,
        layer_id: usize,
        node_x: usize,
        node_y: usize,
        node_z: usize,
    ) -> bool {
        self.bins
            .borrow()
            .contains_key(&(layer_id, node_x, node_y, node_z))
    }

    /// Get the bin path for a given coordinate
    pub fn bin_path_for(&self, x: usize, y: usize, z: usize, layer_id: usize) -> String {
        let brick_x = x / self.brick_size[0];
        let brick_y = y / self.brick_size[1];
        let brick_z = z / self.brick_size[2];

        let node_x = brick_x / self.node_size[0] * self.node_size[0];
        let node_y = brick_y / self.node_size[1] * self.node_size[1];
        let node_z = brick_z / self.node_size[2] * self.node_size[2];

        format!(
            "variables/{}/0/bundles/0/{}-{}-{}.bin",
            layer_id, node_x, node_y, node_z
        )
    }

    /// Get all bin paths needed for a plane
    pub fn bin_paths_for_plane(&self, z: usize, layer_id: usize) -> Vec<String> {
        let x_len = self.layer.volumes()[0].dimensions()[0].size();
        let y_len = self.layer.volumes()[0].dimensions()[1].size();

        let mut paths = Vec::new();
        let mut seen = HashMap::new(); //TODO: use HashSet

        for x in (0..x_len).step_by(self.brick_size[0] * self.node_size[0]) {
            for y in (0..y_len).step_by(self.brick_size[1] * self.node_size[1]) {
                let path = self.bin_path_for(x, y, z, layer_id);
                if !seen.contains_key(&path) {
                    seen.insert(path.clone(), ());
                    paths.push(path);
                }
            }
        }

        paths
    }
}

/// File-based constructor
#[cfg(feature = "std")]
impl VoxelLayer<FileLoader> {
    pub fn from_file(path: &str) -> Self {
        let layer = Layer::from_file(&format!("{}/layer.json", path));
        let loader = FileLoader::new(path);
        Self::with_loader(layer, loader)
    }
}

/// Memory-based constructor
impl VoxelLayer<MemoryLoader> {
    pub fn from_layer(layer: Layer) -> Self {
        Self::with_loader(layer, MemoryLoader::new())
    }

    pub fn from_layer_json(json: &str) -> Self {
        let layer = Layer::from_str(json);
        Self::with_loader(layer, MemoryLoader::new())
    }
}

/// Accessors
impl<L: BinLoader> VoxelLayer<L> {
    pub fn layer(&self) -> &Layer {
        &self.layer
    }

    /// get all the x,y coordinates of the data
    /// used for elevation querying
    pub fn get_xy(&self) -> Vec<(f32, f32)> {
        let x_vals = self.layer().volumes()[0].dimensions()[0].values();
        let y_vals = self.layer().volumes()[0].dimensions()[1].values();

        x_vals
            .iter()
            .flat_map(|x| y_vals.iter().map(|y| (*x, *y)))
            .collect()
    }
}

/// Query data
impl<L: BinLoader> VoxelLayer<L> {
    /// Convert a world/projected coordinate to a grid index.
    ///
    /// Vertical exaggeration is a rendering-only stretch applied when drawing the volume;
    /// it is not part of the stored grid geometry, so it is not applied here.
    #[must_use]
    pub fn indices_for_geometry(&self, x: f32, y: f32, z: f32) -> (usize, usize, usize) {
        let x_index = self.layer().volumes()[0].dimensions()[0].find(x, None);
        let y_index = self.layer().volumes()[0].dimensions()[1].find(y, None);
        let z_index = self.layer().volumes()[0].dimensions()[2].find(z, None);

        (x_index, y_index, z_index)
    }

    /// The node-origin key (in brick-index units) that a grid index falls in.
    /// This is the same key used by the bin cache, `load_bin`/`preload_bin`, and `bin_path_for`.
    #[must_use]
    pub fn node_key_for_indices(&self, x: usize, y: usize, z: usize) -> (usize, usize, usize) {
        let brick_x = x / self.brick_size[0];
        let brick_y = y / self.brick_size[1];
        let brick_z = z / self.brick_size[2];

        let node_x = brick_x / self.node_size[0] * self.node_size[0];
        let node_y = brick_y / self.node_size[1] * self.node_size[1];
        let node_z = brick_z / self.node_size[2] * self.node_size[2];

        (node_x, node_y, node_z)
    }

    /// The node-origin key that a world/projected coordinate falls in.
    #[must_use]
    pub fn node_key_for_geometry(&self, x: f32, y: f32, z: f32) -> (usize, usize, usize) {
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

    /// Drop a single cached node, freeing its memory.
    pub fn evict_bin(&self, layer_id: usize, node_x: usize, node_y: usize, node_z: usize) {
        self.bins
            .borrow_mut()
            .remove(&(layer_id, node_x, node_y, node_z));
    }

    /// Drop every cached node, freeing all bin memory held by this layer.
    pub fn clear_bins(&self) {
        self.bins.borrow_mut().clear();
    }

    /// Returns `None` if the queried voxel holds the variable's declared no-data value.
    pub fn query_geometry(&self, x: f32, y: f32, z: f32, layer_id: usize) -> Option<f32> {
        let (x_index, y_index, z_index) = self.indices_for_geometry(x, y, z);
        self.query(x_index, y_index, z_index, layer_id)
    }

    /// Returns `None` if the queried voxel holds the variable's declared no-data value.
    #[inline]
    pub fn query(&self, x: usize, y: usize, z: usize, layer_id: usize) -> Option<f32> {
        let (node_x, node_y, node_z) = self.node_key_for_indices(x, y, z);

        // Lazy load the bin if needed
        self.load_bin(layer_id, node_x, node_y, node_z);

        let bins = self.bins.borrow();
        let bin_data = bins
            .get(&(layer_id, node_x, node_y, node_z))
            .unwrap_or_else(|| panic!("Bin data not found for {node_x},{node_y},{node_z}"));

        let raw = bin_data.query(x, y, z);
        match self.layer.no_data_for(layer_id) {
            Some(no_data) if raw == no_data => None,
            _ => Some(raw),
        }
    }

    // z is the index of the plane, find all the values in that plane. `None` entries are
    // voxels holding the variable's declared no-data value.
    pub fn query_plane(&self, z: i32) -> Result<Vec<Option<f32>>, String> {
        let x_len = self.layer().volumes()[0].dimensions()[0].size();
        let y_len = self.layer().volumes()[0].dimensions()[1].size();
        let z_len = self.layer().volumes()[0].dimensions()[2].size();
        let z = z.clamp(0, (z_len - 1) as i32) as usize;

        Ok((0..x_len)
            .flat_map(|x| (0..y_len).map(move |y| self.query(x, y, z, 0)))
            .collect())
    }

    /// `None` entries are voxels holding the variable's declared no-data value.
    pub fn query_arr(&self, arr: &[usize], z: i32) -> Result<Vec<Option<f32>>, String> {
        let x_len = self.layer().volumes()[0].dimensions()[0].size();
        let y_len = self.layer().volumes()[0].dimensions()[1].size();
        let z_len = self.layer().volumes()[0].dimensions()[2].size();

        let r = arr.iter().enumerate().map(|(i, v)| {
            // x1 and y1 are lat lon so need to be flipped
            let x1 = i / x_len;
            let y1 = i % y_len;
            let z1 = (z + (*v as i32)).clamp(0, (z_len - 1) as i32) as usize;
            self.query(x1, y1, z1, 0)
        });

        Ok(r.collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let queried_key = layer.node_key_for_indices(ix, iy, iz);
        assert_eq!(planned_key, queried_key);

        assert!(!layer.is_bin_loaded(layer_id, planned_key.0, planned_key.1, planned_key.2));
        let _value = layer.query_geometry(x, y, z, layer_id);
        assert!(layer.is_bin_loaded(layer_id, planned_key.0, planned_key.1, planned_key.2));

        layer.evict_bin(layer_id, planned_key.0, planned_key.1, planned_key.2);
        assert!(!layer.is_bin_loaded(layer_id, planned_key.0, planned_key.1, planned_key.2));

        let _value = layer.query_geometry(x, y, z, layer_id);
        layer.clear_bins();
        assert!(!layer.is_bin_loaded(layer_id, planned_key.0, planned_key.1, planned_key.2));
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

        // must not panic
        let _ = layer.query_geometry(x, y, z, 0);
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
}
