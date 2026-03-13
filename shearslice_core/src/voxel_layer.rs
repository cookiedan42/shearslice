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

/// bin key is layer xyz
impl VoxelLayer {
    #[cfg(feature = "std")]
    pub fn from_file(path: &str) -> Self {
        let layer = Layer::from_file(&format! {"{path}/layer.json"});

        let node_size = layer.index().node_size();
        let brick_size = layer.index().brick_size();
        let mut bins = HashMap::new();

        // Get volume dimensions from layer
        let volumes = layer.volumes();
        if let Some(volume) = volumes.first() {
            let dims = volume.dimensions();

            let files_x = dims[0].size() / brick_size[0] / node_size[0];
            let files_y = dims[1].size() / brick_size[1] / node_size[1];
            let files_z = dims[2].size() / brick_size[2] / node_size[2];

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

    pub fn new(layer: Layer, bins: HashMap<(usize, usize, usize, usize), Vec<u8>>) -> Self {
        Self { layer, bins }
    }
}

impl VoxelLayer {
    pub fn layer(&self) -> &Layer {
        &self.layer
    }
}

impl VoxelLayer {
    // x,y,z are indices, find the brick they belong to and query the brick
    pub fn query(&self, x: usize, y: usize, z: usize, layer_id: usize) -> f32 {
        let brick_size = self.layer().index().brick_size();
        let node_size = self.layer().index().node_size();

        let brick_x = x / brick_size[0]; // 0-31
        let brick_y = y / brick_size[1]; // 0-31
        let brick_z = z / brick_size[2]; // 0-31

        let file_x = brick_x / node_size[0] * node_size[0]; // 0-28, spacing 4
        let file_y = brick_y / node_size[1] * node_size[1]; // 0-28, spacing 4
        let file_z = brick_z / node_size[2] * node_size[2]; // 0

        // TODO: handle the edge case when x and y are not perfect multiples of node size * brick size

        let local_brick_x = brick_x % node_size[0]; //0 - 3
        let local_brick_y = brick_y % node_size[1]; //0 - 3
        let local_brick_z = brick_z % node_size[2]; //0 - 3

        let local_voxel_x = x % brick_size[0]; // 0-31
        let local_voxel_y = y % brick_size[1]; // 0-31
        let local_voxel_z = z % brick_size[2]; // 0-31

        let file_offset = 24 // 6 byte header
            + 4 * (
                // offset by completed bricks
                local_brick_z * (34 * 34 * 34) * 4 * 4
                + local_brick_y * (34 * 34 * 34) * 4
                + local_brick_x * (34 * 34 * 34)
                // offset within brick
                + (local_voxel_z) * 34 * 34
                + (local_voxel_y) * 34
                + (local_voxel_x)
            );

        let bin_data = self
            .bins
            .get(&(layer_id, file_x, file_y, file_z))
            .expect(format!("Bin data not found for {file_x},{file_y},{file_z}").as_str());

        // Read 4 bytes at the offset
        let buffer: [u8; 4] = bin_data[file_offset..file_offset + 4].try_into().unwrap();

        let value = f32::from_le_bytes(buffer);

        value
    }

    // z is the index of the plane, find all the values in that plane
    pub fn query_plane(&self, z: usize) -> Result<Vec<f32>, String> {
        Ok((0..1024)
            .map(|x| (0..1024).map(move |y| self.query(x, y, z, 0)))
            .flatten()
            .collect())
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
