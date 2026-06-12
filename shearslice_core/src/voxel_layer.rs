use crate::layer::Layer;
use crate::node;
use hashbrown::HashMap;
use std::cell::RefCell;

pub struct VoxelLayer {
    layer: Layer,
    path: String,
    bins: RefCell<HashMap<(usize, usize, usize, usize), node::Node<f32>>>,
    // Cached values
    brick_size: [usize; 3],
    node_size: [usize; 3],
    apron_width: usize,
}

/// Constructors
impl VoxelLayer {
    #[cfg(feature = "std")]
    pub fn from_file(path: &str) -> Self {
        let layer = Layer::from_file(&format! {"{path}/layer.json"});

        let node_size = layer.index().node_size();
        let brick_size = layer.index().brick_size();
        let apron_width = layer.index().apron_width();

        Self {
            layer,
            path: path.to_string(),
            bins: RefCell::new(HashMap::new()),
            brick_size,
            node_size,
            apron_width,
        }
    }

    /// HashMap Keys are (layer_id, file_x, file_y, file_z)
    /// initialize from a partially loaded layer
    pub fn new(layer: Layer, bins: HashMap<(usize, usize, usize, usize), node::Node<f32>>) -> Self {
        let node_size = layer.index().node_size();
        let brick_size = layer.index().brick_size();
        let apron_width = layer.index().apron_width();

        Self {
            layer,
            path: String::new(),
            bins: RefCell::new(bins),
            brick_size,
            node_size,
            apron_width,
        }
    }

    /// Load a bin file lazily
    #[cfg(feature = "std")]
    fn load_bin(&self, layer_id: usize, node_x: usize, node_y: usize, node_z: usize) {
        use std::fs;

        let key = (layer_id, node_x, node_y, node_z);

        // Already loaded
        if self.bins.borrow().contains_key(&key) {
            return;
        }

        let bin_path = format!(
            "{}/variables/{}/0/bundles/0/{}-{}-{}.bin",
            self.path, layer_id, node_x, node_y, node_z
        );

        let contents =
            fs::read(&bin_path).unwrap_or_else(|_| panic!("Failed to read bin file: {bin_path}"));

        let node = node::Node::from_bin(
            &contents,
            self.node_size[0],
            self.node_size[1],
            self.node_size[2],
            self.brick_size[0],
            self.brick_size[1],
            self.brick_size[2],
            self.apron_width,
        );

        self.bins.borrow_mut().insert(key, node);
    }
}

/// Accessors
impl VoxelLayer {
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
impl VoxelLayer {
    pub fn query_geometry(&self, x: f32, y: f32, z: f32, layer_id: usize) -> f32 {
        let vertical_exaggeration =
            self.layer().styles().volume_styles()[0].vertical_exaggeration();

        let x_index = self.layer().volumes()[0].dimensions()[0].find(x, None);
        let y_index =
            self.layer().volumes()[0].dimensions()[1].find(y, Some(vertical_exaggeration));
        let z_index = self.layer().volumes()[0].dimensions()[2].find(z, None);

        self.query(x_index, y_index, z_index, layer_id)
    }

    // x,y,z are indices, find the brick they belong to and query the brick
    #[inline]
    pub fn query(&self, x: usize, y: usize, z: usize, layer_id: usize) -> f32 {
        let brick_size = self.layer().index().brick_size();
        let node_size = self.layer().index().node_size();

        let brick_x = x / brick_size[0]; // 0-31
        let brick_y = y / brick_size[1]; // 0-31
        let brick_z = z / brick_size[2]; // 0-31

        let node_x = brick_x / node_size[0] * node_size[0]; // 0-28, spacing 4
        let node_y = brick_y / node_size[1] * node_size[1]; // 0-28, spacing 4
        let node_z = brick_z / node_size[2] * node_size[2]; // 0

        // Lazy load the bin file if needed
        // assume no lazy loading if not using std
        #[cfg(feature = "std")]
        self.load_bin(layer_id, node_x, node_y, node_z);

        let bins = self.bins.borrow();
        let bin_data = bins
            .get(&(layer_id, node_x, node_y, node_z))
            .unwrap_or_else(|| panic!("Bin data not found for {node_x},{node_y},{node_z}"));

        bin_data.query(x, y, z)
    }

    // z is the index of the plane, find all the values in that plane
    pub fn query_plane(&self, z: i32) -> Result<Vec<f32>, String> {
        let x_len = self.layer().volumes()[0].dimensions()[0].size();
        let y_len = self.layer().volumes()[0].dimensions()[1].size();
        let z_len = self.layer().volumes()[0].dimensions()[2].size();
        let z = z.clamp(0, (z_len - 1) as i32) as usize;

        Ok((0..x_len)
            .flat_map(|x| (0..y_len).map(move |y| self.query(x, y, z, 0)))
            .collect())
    }

    pub fn query_arr(&self, arr: &[usize], z: i32) -> Result<Vec<f32>, String> {
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
}
