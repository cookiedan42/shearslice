mod layer;
mod node;
pub use node::Node;
mod png;
mod variable_index;
mod voxel_layer;
pub use voxel_layer::{BinLoader, FileLoader, MemoryLoader, VoxelLayer};

pub use layer::Layer;
pub use png::to_png;
pub use variable_index::Index;

/*

1. use serde to serialize the json files
2. add methods to identify relevant .bin files and slice

v0 query a single coordinate without interpolation
v1 query a single coordinate with interpolation
v2 do iterative query over a plane
v3 consider batching reads into the same file
v4 rayon per file?

*/

// for each horizontal cross section, create a 1024 by 1024 png
// encode the values using the provided colormap in slpk
// write the png to a file based on z value

#[cfg(test)]
mod tests {
    use crate::voxel_layer::VoxelLayer;
    use rstest::rstest;

    /// compare coordinate by coordinate reading of the slpk file against the netcdf file
    #[rstest]
    #[case("30")]
    #[case("256")]
    #[case("768")]
    #[case("1024")]
    fn test_coord(#[case] name: &str) {
        let layer = VoxelLayer::from_file(&format!("../data/{name}"));
        let netcdf_file = netcdf::open(&format!("../data/2D_Result_{name}.nc")).unwrap();

        let windspeed_var = netcdf_file.variable("WindSpeed").unwrap();
        // lon,lat,z,t
        windspeed_var.get_value::<f32, _>([0, 0, 0, 0]).unwrap();
        let dims = windspeed_var
            .dimensions()
            .into_iter()
            .map(|dim| dim.len())
            .collect::<Vec<_>>();
        let t_size = dims[0];
        let z_size = dims[1];
        let x_size = dims[2];
        let y_size = dims[3];
        println!("t_size: {t_size}, x_size: {x_size}, y_size: {y_size}, z_size: {z_size}");

        for t in 0..t_size {
            for z in 0..z_size {
                for y in 0..y_size {
                    for x in 0..x_size {
                        let windspeed_nc = windspeed_var.get_value::<f32, _>([t, z, y, x]).unwrap();
                        let windspeed_slpk = layer.query(x, y, z, 0);
                        assert_eq!(windspeed_nc, windspeed_slpk);
                    }
                }
            }
        }
    }
}
