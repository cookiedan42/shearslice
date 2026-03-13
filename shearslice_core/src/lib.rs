mod layer;
mod png;
mod variable_index;
mod voxel_layer;

pub use layer::Layer;
pub use variable_index::Index;
pub use voxel_layer::VoxelLayer;

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
    use crate::png::{ColorRamp, to_bmp, to_gif, to_png, write_png_to_file};
    use crate::voxel_layer::VoxelLayer;

    #[test]
    fn test_plane() {
        let layer = VoxelLayer::from_file("../data/f773");

        let mut buffers = Vec::new();

        for z in 0..49 {
            let res = layer.query_plane(z).unwrap();
            assert!(res.len() == 1024 * 1024);

            let img_data = to_png(&res, ColorRamp::Inferno);

            write_png_to_file(&img_data, format! {"../data/img/{}.png",z}).unwrap();

            println!("Wrote png {z} to file");

            buffers.push(img_data);
        }

        to_gif(buffers, "../data/img/all.gif");
        println!("Wrote gif to file");
    }
}
