mod layer;
mod png;
mod variable_index;
mod voxel_layer;

pub use layer::Layer;
pub use png::to_png;
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
    use crate::png::{ColorRamp, to_bmp, to_buffer, to_gif, write_png_to_file};
    use crate::voxel_layer::VoxelLayer;

    #[test]
    fn test_plane() {
        let layer = VoxelLayer::from_file("../data/f773");

        let mut buffers = Vec::new();

        let [s, e] = layer.layer().styles().variable_styles()[0]
            .transfer_function()
            .stretch_range();

        for z in 0..49 {
            let res = layer.query_plane(z).unwrap();
            assert!(res.len() == 1024 * 1024);

            // normalise data
            let res1: Vec<f32> = res
                .iter()
                .map(|v| v.clamp(*s, *e))
                .map(|v| (v - *s) / (*e - *s))
                // .map(|v| (v - min_value) / (dist_max_value - dist_min_value))
                // .map(|v| (v - min_value) / (dist_max_value - dist_min_value))
                .collect();

            println!(
                "min: {}, max: {}",
                res1.iter().fold(f32::INFINITY, |a, &b| a.min(b)),
                res1.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b))
            );

            let img_data = to_buffer(
                &res1,
                layer.layer().styles().variable_styles()[0]
                    .transfer_function()
                    .color_stops(),
            );

            write_png_to_file(&img_data, format! {"../data/img/{}.png",z}).unwrap();

            println!("Wrote png {z} to file");

            buffers.push(img_data);
        }

        to_gif(buffers, "../data/img/all.gif");
        println!("Wrote gif to file");
    }
}
