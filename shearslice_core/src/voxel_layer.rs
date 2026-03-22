use crate::layer::Layer;
use crate::variable_index::Index;
use core::cmp::max;
use hashbrown::HashMap;

#[cfg(feature = "std")]
use std::fs;
#[cfg(feature = "std")]
use std::fs::File;
#[cfg(feature = "std")]
use std::io::{Read, Seek, SeekFrom};

pub struct VoxelLayer {
    layer: Layer,
    bins: HashMap<(usize, usize, usize, usize), Vec<u8>>,
}

/// Constructors
impl VoxelLayer {
    #[cfg(feature = "std")]
    pub fn from_file(path: &str) -> Self {
        let layer = Layer::from_file(&format! {"{path}/layer.json"});

        let node_size = layer.index().node_size();
        let mut bins = HashMap::new();

        // Get volume dimensions from layer
        let volumes = layer.volumes();
        if let Some(volume) = volumes.first() {
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
                    let contents = fs::read(bin_path.clone()).expect("Failed to read bin file");
                    bins.insert(
                        (
                            var_id,
                            file_x * node_size[0],
                            file_y * node_size[1],
                            file_z * node_size[2],
                        ),
                        contents,
                    );
                }
            }
        }

        Self { layer, bins }
    }

    /// HashMap Keys are (layer_id, file_x, file_y, file_z)
    pub fn new(layer: Layer, bins: HashMap<(usize, usize, usize, usize), Vec<u8>>) -> Self {
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
        let x_index = self.layer().volumes()[0].dimensions()[0].find(x);
        let y_index = self.layer().volumes()[0].dimensions()[1].find(y);
        let z_index = self.layer().volumes()[0].dimensions()[2].find(z);

        self.query(x_index, y_index, z_index, layer_id)
    }

    // x,y,z are indices, find the brick they belong to and query the brick
    pub fn query(&self, x: usize, y: usize, z: usize, layer_id: usize) -> f32 {
        let brick_size = self.layer().index().brick_size();
        let node_size = self.layer().index().node_size();
        let apron_width = self.layer().index().apron_width();

        let skip: [usize; 3] = [
            brick_size[0] + apron_width + apron_width,
            brick_size[1] + apron_width + apron_width,
            brick_size[2] + apron_width + apron_width,
        ];

        let brick_x = x / brick_size[0]; // 0-31
        let brick_y = y / brick_size[1]; // 0-31
        let brick_z = z / brick_size[2]; // 0-31

        let file_x = brick_x / node_size[0] * node_size[0]; // 0-28, spacing 4
        let file_y = brick_y / node_size[1] * node_size[1]; // 0-28, spacing 4
        let file_z = brick_z / node_size[2] * node_size[2]; // 0

        let local_brick_x = brick_x % node_size[0]; //0 - 3
        let local_brick_y = brick_y % node_size[1]; //0 - 3
        let local_brick_z = brick_z % node_size[2]; //0 - 3

        let local_voxel_x = x % brick_size[0]; // 0-31
        let local_voxel_y = y % brick_size[1]; // 0-31
        let local_voxel_z = z % brick_size[2]; // 0-31

        let file_offset = 24 // 24 byte header
          // 4 because f32 is 4 bytes
            + 4 * (
                // offset by completed bricks
                local_brick_z * (skip[0] * skip[1] * skip[2]) * node_size[0] * node_size[1]
                + local_brick_y * (skip[0] * skip[1] * skip[2]) * node_size[0]
                + local_brick_x * (skip[0] * skip[1] * skip[2])
                // offset within brick
                + (local_voxel_z+1) * skip[0] * skip[1]
                + (local_voxel_y+1) * skip[0]
                + (local_voxel_x+1) 
            );

        let bin_data = self
            .bins
            .get(&(layer_id, file_x, file_y, file_z))
            .expect(format!("Bin data not found for {file_x},{file_y},{file_z}").as_str());

        // Read 4 bytes at the offset
        let buffer: [u8; 4] = bin_data[file_offset..file_offset + 4].try_into().unwrap();

        f32::from_le_bytes(buffer)
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
            let x1 = i / 1024;
            let y1 = i % 1024;
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
