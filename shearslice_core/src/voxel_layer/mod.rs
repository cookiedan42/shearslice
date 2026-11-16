use crate::bin_reader::MemoryBinReader;
use crate::layer::Layer;
use crate::loaders::{BinLoader, MemoryLoader};
use crate::node;
use core::cell::RefCell;
use hashbrown::HashMap;
use std::rc::Rc;

mod file_voxel_layer;
mod memory_voxel_layer;
pub use memory_voxel_layer::MemoryVoxelLayer;

#[cfg(feature = "std")]
use crate::loaders::EslpkLoader;
#[cfg(feature = "std")]
pub use file_voxel_layer::FileVoxelLayer;

pub trait VoxelLayer {
    fn layer(&self) -> &Layer;

    fn query(&self, x: usize, y: usize, z: usize, layer_id: usize) -> Option<f32>;

    /// `None` entries are voxels holding the variable's declared no-data value.
    fn query_arr(&self, arr: &[usize], z: i32) -> Result<Vec<Option<f32>>, String> {
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

    // z is the index of the plane, find all the values in that plane. `None` entries are
    // voxels holding the variable's declared no-data value.
    fn query_plane(&self, z: i32) -> Result<Vec<Option<f32>>, String> {
        let x_len = self.layer().volumes()[0].dimensions()[0].size();
        let y_len = self.layer().volumes()[0].dimensions()[1].size();
        let z_len = self.layer().volumes()[0].dimensions()[2].size();
        let z = z.clamp(0, (z_len - 1) as i32) as usize;

        Ok((0..x_len)
            .flat_map(|x| (0..y_len).map(move |y| self.query(x, y, z, 0)))
            .collect())
    }
}

pub struct ImplVoxelLayer<L: BinLoader> {
    layer: Layer,
    loader: L,
    bins: RefCell<HashMap<(usize, usize, usize, usize), Rc<dyn crate::bin_reader::BinReader>>>,
    nodes: RefCell<HashMap<(usize, usize, usize, usize), node::Node<f32>>>,
    pub brick_size: [usize; 3],
    pub node_size: [usize; 3],
    pub apron_width: usize,
}

/// Constructors
impl<L: BinLoader> ImplVoxelLayer<L> {
    /// Create a new ImplVoxelLayer with a custom loader
    pub fn with_loader(layer: Layer, loader: L) -> Self {
        let node_size = layer.index().node_size();
        let brick_size = layer.index().brick_size();
        let apron_width = layer.index().apron_width();

        Self {
            layer,
            loader,
            bins: RefCell::new(HashMap::new()),
            nodes: RefCell::new(HashMap::new()),
            brick_size,
            node_size,
            apron_width,
        }
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

        let node = node::Node::from_bin(
            contents.as_ref(),
            self.node_size,
            self.brick_size[0],
            self.brick_size[1],
            self.brick_size[2],
            self.apron_width,
        );

        self.bins.borrow_mut().insert(key, contents);
        self.nodes.borrow_mut().insert(key, node);
    }

    /// Preload a specific bin
    /// internal mutation of `self.bins` and `self.nodes`
    pub fn preload_bin(
        &self,
        layer_id: usize,
        node_x: usize,
        node_y: usize,
        node_z: usize,
        data: Vec<u8>,
    ) {
        let key = (layer_id, node_x, node_y, node_z);
        let bin = Rc::new(MemoryBinReader::new(data.into()));

        let node = node::Node::from_bin(
            bin.as_ref(),
            self.node_size,
            self.brick_size[0],
            self.brick_size[1],
            self.brick_size[2],
            self.apron_width,
        );
        self.bins.borrow_mut().insert(key, bin);
        self.nodes.borrow_mut().insert(key, node);
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

/// Memory-based constructor
impl ImplVoxelLayer<MemoryLoader> {
    pub fn from_layer(layer: Layer) -> Self {
        Self::with_loader(layer, MemoryLoader::new())
    }

    pub fn from_layer_json(json: &str) -> Self {
        let layer = Layer::from_str(json);
        Self::with_loader(layer, MemoryLoader::new())
    }
}

/// Accessors
impl<L: BinLoader> ImplVoxelLayer<L> {
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
impl<L: BinLoader> ImplVoxelLayer<L> {
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

    /// Returns `None` if `(x, y, z)` falls outside the layer's extent, or if the queried
    /// voxel holds the variable's declared no-data value.
    /// takes world coordinates in same units as layer
    pub fn query_geometry(&self, x: f32, y: f32, z: f32, layer_id: usize) -> Option<f32> {
        if !self.in_extent(x, y, z) {
            return None;
        }
        let (x_index, y_index, z_index) = self.indices_for_geometry(x, y, z);
        self.query(x_index, y_index, z_index, layer_id)
    }

    /// Returns `None` if `(x, y, z)` falls outside the grid's dimensions, or if the queried
    /// voxel holds the variable's declared no-data value.
    /// takes index of the grid cell in the layer's grid
    #[inline]
    pub fn query(&self, x: usize, y: usize, z: usize, layer_id: usize) -> Option<f32> {
        let dims = self.layer().volumes()[0].dimensions();
        if x >= dims[0].size() || y >= dims[1].size() || z >= dims[2].size() {
            return None;
        }

        let (node_x, node_y, node_z) = self.node_key_for_indices(x, y, z);

        // Lazy load the bin if needed
        self.load_bin(layer_id, node_x, node_y, node_z);

        let bins = self.bins.borrow();
        let bin_data = bins
            .get(&(layer_id, node_x, node_y, node_z))
            .unwrap_or_else(|| panic!("Bin data not found for {node_x},{node_y},{node_z}"));

        let raw = self
            .nodes
            .borrow()
            .get(&(layer_id, node_x, node_y, node_z))
            .unwrap_or_else(|| panic!("Bin data not found for {node_x},{node_y},{node_z}"))
            .query(x, y, z, bin_data.as_ref());

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
