mod layer;
mod variable_index;
mod voxel_layer;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
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
