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
    capabilities: Option<Vec<String>>,
}

impl Layer {
    pub fn from_file(path: &str) -> Self {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }
    pub fn from_str(json: &str) -> Self {
        serde_json::from_str(json).unwrap()
    }
    pub fn extent(&self) -> &Extent {
        &self.full_extent
    }

    pub(crate) fn variables(&self) -> &[Variable] {
        self.variables.as_slice()
    }

    pub(crate) fn volumes(&self) -> &[Volume] {
        &self.volumes
    }

    pub fn index(&self) -> &LayerIndex {
        &self.index
    }
    pub fn styles(&self) -> &Style {
        &self.style
    }

    /// return the 3 indices of the node files for a given variable id
    pub fn tails(&self, _var_id: usize) -> Vec<(usize, usize, usize)> {
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
pub struct Style {
    volume_styles: Vec<VolumeStyle>,
    current_variable_id: u32,
    variable_styles: Vec<VariableStyle>,
}
impl Style {
    pub(crate) fn volume_styles(&self) -> &[VolumeStyle] {
        &self.volume_styles
    }
    pub fn current_variable_id(&self) -> u32 {
        self.current_variable_id
    }
    pub fn variable_styles(&self) -> &[VariableStyle] {
        &self.variable_styles
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct VariableStyle {
    variable_id: u32,
    transfer_function: TransferFunction,
}
impl VariableStyle {
    pub fn variable_id(&self) -> u32 {
        self.variable_id
    }
    pub fn transfer_function(&self) -> &TransferFunction {
        &self.transfer_function
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct TransferFunction {
    interpolation: String, // "linear"
    stretch_range: [f32; 2],
    color_stops: ColorRamp,
}
impl TransferFunction {
    pub fn interpolation(&self) -> &str {
        &self.interpolation
    }
    pub fn stretch_range(&self) -> &[f32; 2] {
        &self.stretch_range
    }
    pub fn color_stops(&self) -> &ColorRamp {
        &self.color_stops
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(transparent)]
pub struct ColorRamp {
    color_stops: Vec<ColorStop>,
}

impl ColorRamp {
    pub fn eval(&self, value: f32) -> [u8; 4] {
        debug_assert!(value >= 0.0 && value <= 1.0);

        match self
            .color_stops
            .binary_search_by(|a| a.position.total_cmp(&value))
        {
            Ok(idx) => self.color_stops[idx].color,
            Err(idx) => {
                if idx == 0 {
                    self.color_stops[0].color
                } else if idx >= self.color_stops.len() {
                    self.color_stops[self.color_stops.len() - 1].color
                } else {
                    // Return closest
                    if (self.color_stops[idx].position - value).abs()
                        < (self.color_stops[idx - 1].position - value).abs()
                    {
                        self.color_stops[idx].color
                    } else {
                        self.color_stops[idx - 1].color
                    }
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ColorStop {
    color: [u8; 4], // rgba
    position: f32,  // 0.0 - 1.0
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VolumeStyle {
    volume_id: u32,
    vertical_exaggeration: f32,
}
impl VolumeStyle {
    pub(crate) fn vertical_exaggeration(&self) -> f32 {
        self.vertical_exaggeration
    }
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
pub struct Extent {
    spatial_reference: SpatialReference,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    zmin: f64,
    zmax: f64,
}

impl Extent {
    pub fn xmin(&self) -> f64 {
        self.xmin
    }
    pub fn xmax(&self) -> f64 {
        self.xmax
    }
    pub fn ymin(&self) -> f64 {
        self.ymin
    }
    pub fn ymax(&self) -> f64 {
        self.ymax
    }
    pub fn zmin(&self) -> f64 {
        self.zmin
    }
    pub fn zmax(&self) -> f64 {
        self.zmax
    }
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
    // TODO: probably split Dimension into two untagged enum variants
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
    pub fn values(&self) -> Vec<f32> {
        if let Some(sp) = &self.irregular_spacing {
            return sp.values.clone();
        }
        if let Some(sp) = &self.regular_spacing {
            return (0..self.size)
                .map(|i| sp.offset + sp.scale() * i as f32)
                .collect();
        }
        unreachable!("Should be in one of the two cases above");
    }

    /// find the index closest to the given value
    pub fn find(&self, value: f32, vertical_exaggeration: Option<f32>) -> usize {
        match self.irregular_spacing() {
            Some(_) => self.find_irregular(value, vertical_exaggeration),
            None => self.find_regular(value, vertical_exaggeration),
        }
    }
    fn find_regular(&self, value: f32, vertical_exaggeration: Option<f32>) -> usize {
        let spacing = self.regular_spacing().unwrap();
        if value <= spacing.offset() {
            return 0;
        } else if value >= spacing.at(self.size() - 1, vertical_exaggeration) {
            return self.size() - 1;
        } else {
            // TODO: convert to binary search
            for i in 0..self.size() {
                if value < spacing.at(i, vertical_exaggeration) {
                    continue;
                }
                // compare current value to the next value
                let low = spacing.at(i, vertical_exaggeration);
                let high = spacing.at(i + 1, vertical_exaggeration);
                if value - low < high - value {
                    return i;
                } else {
                    return i + 1;
                }
            }
        }
        unreachable!("Should be in one of the three cases above");
    }
    fn find_irregular(&self, value: f32, _vertical_exaggeration: Option<f32>) -> usize {
        // TODO: implement vertical_exaggeration if/when needed

        let spacing = self.irregular_spacing().unwrap();

        if value <= spacing.at(0) {
            return 0;
        } else if value >= spacing.at(spacing.len() - 1) {
            return spacing.len() - 1;
        }

        match spacing
            .values()
            .binary_search_by(|a| a.partial_cmp(&value).unwrap())
        {
            Ok(idx) => idx,
            Err(idx) => {
                let low = spacing.at(idx);
                let high = spacing.at(idx + 1);
                if value - low < high - value {
                    idx
                } else {
                    idx + 1
                }
            }
        }
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
    values: Vec<f32>,
}
impl IrregularSpacing {
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn at(&self, index: usize) -> f32 {
        self.values[index]
    }
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub(crate) struct RegularSpacing {
    // space between values
    scale: Option<f32>,
    // offset of the first value
    offset: f32,
}
impl RegularSpacing {
    pub fn default_one() -> f32 {
        1.0
    }

    pub fn scale(&self) -> f32 {
        self.scale.unwrap_or(1.0)
    }
    pub fn at(&self, index: usize, vertical_exaggeration: Option<f32>) -> f32 {
        let vertical_exaggeration = vertical_exaggeration.unwrap_or(1.0);

        self.offset + (self.scale() * vertical_exaggeration) * index as f32
    }
    pub fn offset(&self) -> f32 {
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
