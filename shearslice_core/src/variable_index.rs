use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Index {
    volume_size: [u32; 3],
    trees: Vec<Tree>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct Tree {
    tree_node_count: u32,
    tree_children_list_count: u32,
    leaf_brick_count: u32,
}

impl Index {
    pub fn from_file(path: &str) -> Self {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        serde_json::from_str::<Index>(include_str!("../../data/data1/variables/0/index.json"))
            .unwrap();

        let _layer = Index::from_file("../data/data1/variables/0/index.json");
    }
}
