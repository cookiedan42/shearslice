use crate::layer::Layer;
use crate::variable_index::Index;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

pub struct VoxelLayer {
    path: String,
    layer: Layer,
    indices: Vec<Index>,
}

impl VoxelLayer {
    pub fn new(path: &str) -> Self {
        let layer = Layer::from_file(&format! {"{path}/layer.json"});

        // for each variable, create the index
        let indices = layer
            .variables()
            .iter()
            .map(|v| v.id())
            .map(|id| format! {"{path}/variables/{id}/index.json"})
            .map(|path| Index::from_file(&path))
            .collect();

        Self {
            path: path.to_string(),
            layer,
            indices,
        }
    }
}

impl VoxelLayer {
    pub fn from_file(path: &str) -> Self {
        Self::new(path)
    }
}

impl VoxelLayer {
    pub fn layer(&self) -> &Layer {
        &self.layer
    }
}

impl VoxelLayer {
    // x,y,z are indices, find the brick they belong to and query the brick
    pub fn query(&self, x: usize, y: usize, z: usize, layer_id: usize) -> Result<f32, String> {
        let brick_size = self.layer().index().brick_size();
        let node_size = self.layer().index().node_size();

        let brick_x = x / brick_size[0];
        let brick_y = y / brick_size[1];
        let brick_z = z / brick_size[2];

        let file_x = brick_x / node_size[0];
        let file_y = brick_y / node_size[1];
        let file_z = brick_z / node_size[2];

        // find the correct file
        let bin_path = format!(
            "{}/variables/{}/0/bundles/0/{}-{}-{}.bin",
            self.path,
            layer_id,
            file_x * node_size[0],
            file_y * node_size[1],
            file_z * node_size[2]
        );

        // TODO: handle the edge case when x and y are not perfect multiples of node size * brick size

        let local_brick_x = brick_x % node_size[0]; //0 - 3
        let local_brick_y = brick_y % node_size[1]; //0 - 3
        let local_brick_z = brick_z % node_size[2]; //0 - 3

        let local_voxel_x = x % brick_size[0]; // 0-31
        let local_voxel_y = y % brick_size[1]; // 0-31
        let local_voxel_z = z % brick_size[2]; // 0-31

        // sum all bricks before me
        // add my current offset

        let file_offset = 4
            * (
                // offset by completed bricks
                  local_brick_z * (34 * 34 * 34) *4*4
                + local_brick_y * (34 * 34 * 34) *4
                + local_brick_x * (34 * 34 * 34)*1
                // offset within brick
                + (local_voxel_z+1) * 34 * 34 
                + (local_voxel_y+1) * 34 
                + (local_voxel_x+1)
            );
        
        // off by 1 in each dimension because starting cell has 0 front apron



        let mut file =
            File::open(&bin_path).map_err(|e| format!("Failed to open {}: {}", bin_path, e))?;
        file.seek(SeekFrom::Start(file_offset as u64))
            .map_err(|e| format!("Failed to seek: {}", e))?;

        let mut buffer = [0u8; 4];
        file.read_exact(&mut buffer)
            .map_err(|e| format!("Failed to read data: {}", e))?;

        let value = f32::from_le_bytes(buffer);

        Ok(value)
    }

    // z is the index of the plane, find all the values in that plane
    pub fn query_plane(&self, z: usize) -> Result<Vec<f32>, String> {
        Ok((0..1024)
            .map(|x| (0..1024).map(move |y| self.query(x, y, z, 0).unwrap()))
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
