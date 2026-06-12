use shearslice_core::VoxelLayer;

fn main() {
    // Load the voxel layer
    let layer = VoxelLayer::from_file("../data/1024");

    // Run queries to generate meaningful flame data
    let x_len = layer.layer().volumes()[0].dimensions()[0].size();
    let y_len = layer.layer().volumes()[0].dimensions()[1].size();
    let z_len = layer.layer().volumes()[0].dimensions()[2].size();

    // Query all planes
    for z in 0..z_len {
        let _ = layer.query_plane(z as i32);
    }

    let mut a = vec![0_f32; 1024 * 1024 * 50];
    // Query individual points
    for z in 0..z_len {
        for y in 0..y_len {
            for x in 0..x_len {
                a[x + y * 1024 + z * 1024 * 1024] = layer.query(x, y, z, 0);
            }
        }
    }
    println!("done: {:?}", a.iter().sum::<f32>());
}
