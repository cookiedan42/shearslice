use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Layer {
    layer_type: LayerType,
    version: String, // string of float
    name: String,
    spatial_reference: SpatialReference,
    full_extent: Extent,
    volumes: Vec<Volume>,
    variables: Vec<Variable>,
    index: Index,
    style: Style,
}

impl Layer {
    pub fn from_file(path: &str) -> Self {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    pub(crate) fn variables(&self) -> &[Variable] {
        self.variables.as_slice()
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
struct Index {
    brick_size: [u32; 3],
    node_size: [u32; 3],
    apron_width: u32,
    max_lod_level: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Variable {
    id: u32,
    name: String,
    description: String,
    unit: String,
    original_format: Format,
    rendering_format: Format,
}

impl Variable {
    pub fn id(&self) -> u32 {
        self.id
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
struct Volume {
    id: u32,
    dimensions: Vec<Dimension>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct Dimension {
    name: String,
    label: String,
    unit: String,
    size: u32,
    irregular_spacing: Option<IrregularSpacing>,
    regular_spacing: Option<RegularSpacing>,
    quantity: Option<Quantity>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
enum Quantity {
    HorizontalCoordinate,
    VerticalCoordinate,
    Time,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IrregularSpacing {
    values: Vec<f64>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegularSpacing {
    scale: f64,
    offset: f64,
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
