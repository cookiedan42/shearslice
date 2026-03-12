use crate::layer::Layer;
use crate::variable_index::Index;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let _layer = VoxelLayer::from_file("../data/data1");
    }
}
