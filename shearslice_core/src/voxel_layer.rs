use crate::layer::Layer;
use crate::node;
use hashbrown::HashMap;

pub struct VoxelLayer {
    layer: Layer,
    bins: HashMap<(usize, usize, usize, usize), node::Node<f32>>,
}

/// Constructors
impl VoxelLayer {
    #[cfg(feature = "std")]
    pub fn from_file(path: &str) -> Self {
        use std::fs;

        let layer = Layer::from_file(&format! {"{path}/layer.json"});

        let node_size = layer.index().node_size();
        let brick_size = layer.index().brick_size();
        let apron_width = layer.index().apron_width();

        let mut bins = HashMap::new();

        // Get volume dimensions from layer
        for var in layer.variables().into_iter() {
            let var_id = var.id();
            let paths = layer.tails(var_id);

            for (file_x, file_y, file_z) in paths {
                let bin_path = format!(
                    "{}/variables/{}/0/bundles/0/{}-{}-{}.bin",
                    path,
                    var_id,
                    file_x * node_size[0],
                    file_y * node_size[1],
                    file_z * node_size[2]
                );
                let contents = fs::read(bin_path).expect("Failed to read bin file");
                bins.insert(
                    (
                        var_id,
                        file_x * node_size[0],
                        file_y * node_size[1],
                        file_z * node_size[2],
                    ),
                    node::Node::from_bin(
                        &contents,
                        node_size[0],
                        node_size[1],
                        node_size[2],
                        brick_size[0],
                        brick_size[1],
                        brick_size[2],
                        apron_width,
                    ),
                );
            }
        }

        Self { layer, bins }
    }

    /// HashMap Keys are (layer_id, file_x, file_y, file_z)
    pub fn new(layer: Layer, bins: HashMap<(usize, usize, usize, usize), node::Node<f32>>) -> Self {
        Self { layer, bins }
    }
}

/// Accessors
impl VoxelLayer {
    pub fn layer(&self) -> &Layer {
        &self.layer
    }
    pub fn get_xy(&self) -> Vec<(f32, f32)> {
        let x_vals = self.layer().volumes()[0].dimensions()[0].values();
        let y_vals = self.layer().volumes()[0].dimensions()[1].values();

        x_vals
            .iter()
            .map(|x| y_vals.iter().map(|y| (*x, *y)))
            .flatten()
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
    pub fn query(&self, x: usize, y: usize, z: usize, layer_id: usize) -> f32 {
        let brick_size = self.layer().index().brick_size();
        let node_size = self.layer().index().node_size();

        let brick_x = x / brick_size[0]; // 0-31
        let brick_y = y / brick_size[1]; // 0-31
        let brick_z = z / brick_size[2]; // 0-31

        let node_x = brick_x / node_size[0] * node_size[0]; // 0-28, spacing 4
        let node_y = brick_y / node_size[1] * node_size[1]; // 0-28, spacing 4
        let node_z = brick_z / node_size[2] * node_size[2]; // 0

        let bin_data = self
            .bins
            .get(&(layer_id, node_x, node_y, node_z))
            .expect(format!("Bin data not found for {node_x},{node_y},{node_z}").as_str());

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
        let layer = VoxelLayer::from_file("../data/data1");
    }
}
