mod layer;
mod node;
pub use node::Node;
mod loaders;
mod png;
mod variable_index;
mod voxel_layer;
#[cfg(feature = "std")]
pub use loaders::FileLoader;
pub use loaders::{BinLoader, MemoryLoader};
pub use voxel_layer::VoxelLayer;

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
                        // format1 datasets declare no no-data value, so every read is `Some`.
                        let windspeed_slpk = layer.query(x, y, z, 0);
                        assert_eq!(Some(windspeed_nc), windspeed_slpk);
                    }
                }
            }
        }
    }

    /// Round-trip *world coordinates* (rather than raw grid indices) through
    /// `query_geometry` against the same netcdf oracle. Unlike `test_coord` above, this
    /// exercises `Dimension::find`, which `query_geometry` relies on and `query` bypasses.
    #[rstest]
    #[case("30")]
    #[case("256")]
    fn test_geometry_round_trip(#[case] name: &str) {
        let layer = VoxelLayer::from_file(&format!("../data/{name}"));
        let netcdf_file = netcdf::open(&format!("../data/2D_Result_{name}.nc")).unwrap();
        let windspeed_var = netcdf_file.variable("WindSpeed").unwrap();

        let dims = layer.layer().volumes()[0].dimensions();
        let x_vals = dims[0].values();
        let y_vals = dims[1].values();
        let z_vals = dims[2].values();

        let x_step = (x_vals.len() / 5).max(1);
        let y_step = (y_vals.len() / 5).max(1);
        let z_step = (z_vals.len() / 5).max(1);

        for iz in (0..z_vals.len()).step_by(z_step) {
            for iy in (0..y_vals.len()).step_by(y_step) {
                for ix in (0..x_vals.len()).step_by(x_step) {
                    let (x, y, z) = (x_vals[ix], y_vals[iy], z_vals[iz]);

                    let windspeed_nc = windspeed_var.get_value::<f32, _>([0, iz, iy, ix]).unwrap();
                    // format1 datasets declare no no-data value, so every read is `Some`.
                    let windspeed_slpk = layer.query_geometry(x, y, z, 0).unwrap();
                    assert_eq!(
                        windspeed_nc, windspeed_slpk,
                        "mismatch at ix={ix} iy={iy} iz={iz} (x={x} y={y} z={z})"
                    );
                }
            }
        }
    }

    /// Same comparison as `test_coord`, but against the `format2` sample dataset, which
    /// (unlike every format1 dataset) declares a no-data value and has a volume whose
    /// dimensions aren't exact multiples of brickSize*nodeSize, so it exercises partial
    /// edge nodes. Requires `data/format2` to have been exploded from `data/format2.slpk`
    /// (see TODO.md / README for the extraction command) — `data/format2.slpk` is a ZIP
    /// archive and this reader only reads already-unpacked, gunzipped files.
    #[test]
    fn test_coord_format2() {
        let layer = VoxelLayer::from_file("../data/format2");
        let netcdf_file = netcdf::open("../data/format2.nc").unwrap();

        let windspeed_var = netcdf_file.variable("WindSpeed").unwrap();
        // format2's variable order is [T, WindSpeed, Wind_x, Wind_y, Wind_z, p], so
        // WindSpeed is layer/variable id 1, unlike format1 where it is id 0.
        const WINDSPEED_ID: usize = 1;
        const NO_DATA: f32 = -999.0;

        // netcdf dims are [time, z, lat, lon]. Unlike every format1 dataset, format2's
        // grid is NOT square (lat=172, lon=191), so (unlike `test_coord` above) the lat
        // and lon axes cannot be swapped without going out of bounds -- each must be
        // looped over its own size and passed to the right slot in each API.
        let dims = windspeed_var
            .dimensions()
            .into_iter()
            .map(|dim| dim.len())
            .collect::<Vec<_>>();
        let t_size = dims[0];
        let z_size = dims[1];
        let lat_size = dims[2];
        let lon_size = dims[3];
        println!("t_size: {t_size}, z_size: {z_size}, lat_size: {lat_size}, lon_size: {lon_size}");

        for t in 0..t_size {
            for z in 0..z_size {
                for lat in 0..lat_size {
                    for lon in 0..lon_size {
                        let windspeed_nc =
                            windspeed_var.get_value::<f32, _>([t, z, lat, lon]).unwrap();
                        // SLPK dimension order is [lon, lat, z, time], so query's (x, y)
                        // are (lon, lat).
                        let windspeed_slpk = layer.query(lon, lat, z, WINDSPEED_ID);
                        match windspeed_slpk {
                            None => assert_eq!(
                                windspeed_nc, NO_DATA,
                                "slpk reported no-data but netcdf value at lon={lon} lat={lat} z={z} was {windspeed_nc}"
                            ),
                            Some(v) => {
                                assert_eq!(windspeed_nc, v, "mismatch at lon={lon} lat={lat} z={z}")
                            }
                        }
                    }
                }
            }
        }
    }
}
