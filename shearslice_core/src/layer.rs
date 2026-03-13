use core::cmp::max;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    layer_type: LayerType,
    version: String, // string of float
    name: String,
    spatial_reference: SpatialReference,
    full_extent: Extent,
    volumes: Vec<Volume>,
    variables: Vec<Variable>,
    index: LayerIndex,
    style: Style,
}

impl Layer {
    pub fn from_file(path: &str) -> Self {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }
    pub fn from_str(json: &str) -> Self {
        serde_json::from_str(json).unwrap()
    }

    pub(crate) fn variables(&self) -> &[Variable] {
        self.variables.as_slice()
    }

    pub fn volumes(&self) -> &[Volume] {
        &self.volumes
    }

    pub fn index(&self) -> &LayerIndex {
        &self.index
    }

    pub fn tails(&self, var_id: usize) -> Vec<(usize, usize, usize)> {
        let brick_size = self.index().brick_size();
        let node_size = self.index().node_size();

        // Get volume dimensions from layer
        let volumes = self.volumes();
        let dims = volumes[0].dimensions();

        let files_x = dims[0].size() / brick_size[0] / node_size[0];
        let files_y = dims[1].size() / brick_size[1] / node_size[1];
        let files_z = dims[2].size() / brick_size[2] / node_size[2];

        let mut paths = Vec::new();

        // List actual files by iterating through possible positions
        for file_z in 0..max(files_z, 1) {
            for file_x in 0..max(files_x, 1) {
                for file_y in 0..max(files_y, 1) {
                    paths.push((file_x, file_y, file_z));
                }
            }
        }
        paths
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct Style {
    volume_styles: Vec<VolumeStyle>,
    current_variable_id: u32,
    variable_styles: Vec<VariableStyle>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct VariableStyle {
    variable_id: u32,
    transfer_function: TransferFunction,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct TransferFunction {
    interpolation: String, // "linear"
    stretch_range: [f64; 2],
    color_stops: Vec<ColorStop>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorStop {
    color: [u8; 4], // rgba
    position: f64,  // 0.0 - 1.0
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct VolumeStyle {
    volume_id: u32,
    vertical_exaggeration: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct LayerIndex {
    brick_size: [usize; 3],
    node_size: [usize; 3],
    apron_width: usize,
    max_lod_level: u32,
}

impl LayerIndex {
    pub fn brick_size(&self) -> [usize; 3] {
        self.brick_size
    }

    pub fn node_size(&self) -> [usize; 3] {
        self.node_size
    }
    pub fn apron_width(&self) -> usize {
        self.apron_width
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Variable {
    id: usize,
    name: String,
    description: String,
    unit: String,
    original_format: Format,
    rendering_format: Format,
}

impl Variable {
    pub fn id(&self) -> usize {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Format {
    #[serde(rename = "type")]
    type_: String,
    continuity: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum LayerType {
    Voxel,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct SpatialReference {
    wkid: u32,
    latest_wkid: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct Extent {
    spatial_reference: SpatialReference,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    zmin: f64,
    zmax: f64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Volume {
    id: u32,
    dimensions: Vec<Dimension>,
}
impl Volume {
    pub fn dimensions(&self) -> &[Dimension] {
        &self.dimensions
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Dimension {
    name: String,
    label: String,
    unit: String,
    size: usize,
    irregular_spacing: Option<IrregularSpacing>,
    regular_spacing: Option<RegularSpacing>,
    quantity: Option<Quantity>,
}

impl Dimension {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn size(&self) -> usize {
        self.size
    }
    pub fn regular_spacing(&self) -> Option<RegularSpacing> {
        self.regular_spacing.clone()
    }
    pub fn irregular_spacing(&self) -> Option<IrregularSpacing> {
        self.irregular_spacing.clone()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
enum Quantity {
    HorizontalCoordinate,
    VerticalCoordinate,
    Time,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct IrregularSpacing {
    values: Vec<f64>,
}
impl IrregularSpacing {
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn at(&self, index: usize) -> f64 {
        self.values[index]
    }
    pub fn values(&self) -> &[f64] {
        &self.values
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegularSpacing {
    scale: f64,
    offset: f64,
}
impl RegularSpacing {
    pub fn scale(&self) -> f64 {
        self.scale
    }
    pub fn offset(&self) -> f64 {
        self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        serde_json::from_str::<Layer>(include_str!("../../data/data1/layer.json")).unwrap();

        let _layer = Layer::from_file("../data/data1/layer.json");
    }
}
