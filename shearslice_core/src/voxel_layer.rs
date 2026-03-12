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
    /// Find nearest index in irregular spacing array using binary search
    fn find_index(values: &[f64], target: f64) -> usize {
        match values.binary_search_by(|v| v.partial_cmp(&target).unwrap()) {
            Ok(idx) => idx,
            Err(idx) => {
                if idx == 0 {
                    0
                } else if idx >= values.len() {
                    values.len() - 1
                } else {
                    // Return closest
                    if (values[idx] - target).abs() < (values[idx - 1] - target).abs() {
                        idx
                    } else {
                        idx - 1
                    }
                }
            }
        }
    }

    /// Convert world coordinates to voxel indices
    fn world_to_index(&self, x: f64, y: f64, z: f64) -> (usize, usize, usize) {
        let volume = &self.layer.volumes()[0];
        let dimensions = volume.dimensions();

        // Get lon/lat irregular spacing values
        let lon_values = dimensions
            .iter()
            .find(|d| d.name() == "lon")
            .and_then(|d| d.irregular_spacing())
            .expect("lon dimension not found");

        let lat_values = dimensions
            .iter()
            .find(|d| d.name() == "lat")
            .and_then(|d| d.irregular_spacing())
            .expect("lat dimension not found");

        // Get z regular spacing parameters
        let z_dim = dimensions
            .iter()
            .find(|d| d.name() == "z")
            .expect("z dimension not found");
        let z_spacing = z_dim
            .regular_spacing()
            .expect("z regular spacing not found");
        let z_scale = z_spacing.scale();
        let z_offset = z_spacing.offset();
        let nz = z_dim.size();

        let ix = Self::find_index(lon_values.values(), x);
        let iy = Self::find_index(lat_values.values(), y);
        let iz = ((z - z_offset) / z_scale).round() as i64;
        let iz = iz.max(0).min(nz as i64 - 1) as usize;

        (ix, iy, iz)
    }

    /// Convert voxel index to brick index and local offset
    fn index_to_brick(
        &self,
        ix: usize,
        iy: usize,
        iz: usize,
    ) -> ((usize, usize, usize), (usize, usize, usize)) {
        let brick_size = self.layer.index().brick_size();

        let bx = ix / brick_size[0];
        let by = iy / brick_size[1];
        let bz = iz / brick_size[2];

        let local_x = ix % brick_size[0];
        let local_y = iy % brick_size[1];
        let local_z = iz % brick_size[2];

        ((bx, by, bz), (local_x, local_y, local_z))
    }

    /// Query voxel value at world coordinate
    pub fn query(&self, x: f64, y: f64, z: f64, variable_id: u32) -> Result<f32, String> {
        let volume = &self.layer.volumes()[0];
        let dimensions = volume.dimensions();

        // Get dimension sizes (unused now but kept for potential validation)
        let _nx = dimensions
            .iter()
            .find(|d| d.name() == "lon")
            .map(|d| d.size())
            .unwrap_or(0);
        let _ny = dimensions
            .iter()
            .find(|d| d.name() == "lat")
            .map(|d| d.size())
            .unwrap_or(0);
        let _nz = dimensions
            .iter()
            .find(|d| d.name() == "z")
            .map(|d| d.size())
            .unwrap_or(0);

        // Convert to indices
        let (ix, iy, iz) = self.world_to_index(x, y, z);
        let (brick_idx, local_idx) = self.index_to_brick(ix, iy, iz);

        // Get brick parameters
        let brick_size = self.layer.index().brick_size();
        let apron_width = self.layer.index().apron_width();

        // Calculate brick dimensions with apron
        let brick_with_apron = [
            brick_size[0] + 2 * apron_width,
            brick_size[1] + 2 * apron_width,
            brick_size[2] + 2 * apron_width,
        ];
        let voxels_per_brick = brick_with_apron[0] * brick_with_apron[1] * brick_with_apron[2];

        // Brick file is named by the starting voxel index of the brick
        let brick_start_x = brick_idx.0 * brick_size[0];
        let brick_start_y = brick_idx.1 * brick_size[1];
        let brick_start_z = brick_idx.2 * brick_size[2];

        // Path to binary data - file named as "{start_x}-{start_y}-{start_z}.bin"
        let bin_path = format!(
            "{}/variables/{}/0/bundles/0/{}-{}-{}.bin",
            self.path, variable_id, brick_start_x, brick_start_y, brick_start_z
        );

        //TODO: validate offsets are scoped correctly
        // Calculate offset within brick (accounting for apron)
        let lx = local_idx.0 + apron_width;
        let ly = local_idx.1 + apron_width;
        let lz = local_idx.2 + apron_width;

        let local_offset =
            lz * (brick_with_apron[0] * brick_with_apron[1]) + ly * brick_with_apron[0] + lx;

        // File offset within this single brick file (Float32 = 4 bytes)
        let file_offset = local_offset * 4;

        // Read the value from the binary file
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

    /// Get the spatial extent of the voxel volume
    pub fn get_extent(&self) -> (f64, f64, f64, f64, f64, f64) {
        let volume = &self.layer.volumes()[0];
        let dimensions = volume.dimensions();

        let lon_values = dimensions
            .iter()
            .find(|d| d.name() == "lon")
            .and_then(|d| d.irregular_spacing())
            .expect("lon dimension not found");

        let lat_values = dimensions
            .iter()
            .find(|d| d.name() == "lat")
            .and_then(|d| d.irregular_spacing())
            .expect("lat dimension not found");

        let z_dim = dimensions
            .iter()
            .find(|d| d.name() == "z")
            .expect("z dimension not found");
        let z_spacing = z_dim
            .regular_spacing()
            .expect("z regular spacing not found");
        let z_scale = z_spacing.scale();
        let z_offset = z_spacing.offset();
        let nz = z_dim.size();

        let x_min = lon_values.at(0);
        let x_max = lon_values.at(lon_values.len() - 1);
        let y_min = lat_values.at(0);
        let y_max = lat_values.at(lat_values.len() - 1);
        let z_min = z_offset;
        let z_max = z_offset + (nz as f64 - 1.0) * z_scale;

        (x_min, x_max, y_min, y_max, z_min, z_max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let layer = VoxelLayer::from_file("../data/data1");
        let extent = layer.get_extent();
        println!("Extent: {:?}", extent);

        // Query center point
        let x = (extent.0 + extent.1) / 2.0;
        let y = (extent.2 + extent.3) / 2.0;
        let z = (extent.4 + extent.5) / 2.0;

        if let Ok(value) = layer.query(x, y, z, 0) {
            println!("Value at center: {}", value);
        }
    }
}
