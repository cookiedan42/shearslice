use core::cmp::max;
use serde::{Deserialize, Serialize};
mod style;
pub(crate) use style::*;

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
    #[serde(rename = "popupInfo")]
    popup_info: Option<serde_json::Value>,
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

    pub fn volumes(&self) -> &[Volume] {
        &self.volumes
    }

    pub fn index(&self) -> &LayerIndex {
        &self.index
    }
    pub fn styles(&self) -> &Style {
        &self.style
    }

    /// The declared no-data (fill) value for a variable, by variable id, if any.
    pub fn no_data_for(&self, layer_id: usize) -> Option<f32> {
        self.variables
            .iter()
            .find(|v| v.id() == layer_id)
            .and_then(Variable::no_data)
    }

    /// return the 3 indices of the node files for a given variable id
    pub fn tails(&self, _var_id: usize) -> Vec<(usize, usize, usize)> {
        let brick_size = self.index().brick_size();
        let node_size = self.index().node_size();

        // Get volume dimensions from layer
        let volumes = self.volumes();
        let dims = volumes[0].dimensions();

        // Number of node files along each axis. Both divisions must round up: a volume
        // whose size isn't an exact multiple of brickSize*nodeSize still has a final,
        // partial node file that needs to be listed (e.g. format2's 191-wide x axis with
        // brickSize=32/nodeSize=4 has ceil(191/32)=6 bricks -> ceil(6/4)=2 node files, not
        // the 1 that truncating division would give).
        let files_x = dims[0]
            .size()
            .div_ceil(brick_size[0])
            .div_ceil(node_size[0]);
        let files_y = dims[1]
            .size()
            .div_ceil(brick_size[1])
            .div_ceil(node_size[1]);
        let files_z = dims[2]
            .size()
            .div_ceil(brick_size[2])
            .div_ceil(node_size[2]);

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    unit: Option<String>,
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
    pub fn no_data(&self) -> Option<f32> {
        self.original_format.no_data()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct Format {
    #[serde(rename = "type")]
    type_: String,
    continuity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    no_data: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    has_no_data: Option<bool>,
}

impl Format {
    pub fn no_data(&self) -> Option<f32> {
        self.no_data
    }
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
    wkid: Option<u32>,
    latest_wkid: Option<u32>,
    wkt: Option<String>,
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
pub struct Volume {
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
pub struct Dimension {
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
        let last = self.size() - 1;

        if value <= spacing.offset() {
            return 0;
        }
        if value >= spacing.at(last, vertical_exaggeration) {
            return last;
        }

        // Regular spacing: compute the bracketing indices directly instead of scanning.
        let step = spacing.scale() * vertical_exaggeration.unwrap_or(1.0);
        if step == 0.0 {
            return 0;
        }
        let raw_index = (value - spacing.offset()) / step;
        let low = (raw_index.floor().max(0.0) as usize).min(last);
        let high = (low + 1).min(last);

        let low_value = spacing.at(low, vertical_exaggeration);
        let high_value = spacing.at(high, vertical_exaggeration);
        if value - low_value < high_value - value {
            low
        } else {
            high
        }
    }
    fn find_irregular(&self, value: f32, _vertical_exaggeration: Option<f32>) -> usize {
        // TODO: implement vertical_exaggeration if/when needed

        let spacing = self.irregular_spacing().unwrap();
        let last = spacing.len() - 1;

        if value <= spacing.at(0) {
            return 0;
        }
        if value >= spacing.at(last) {
            return last;
        }

        match spacing
            .values()
            .binary_search_by(|a| a.partial_cmp(&value).unwrap_or(core::cmp::Ordering::Equal))
        {
            Ok(idx) => idx,
            Err(idx) => {
                // `idx` is the insertion point: values[idx - 1] < value < values[idx].
                // Since value is strictly between the first and last elements (checked
                // above), idx is guaranteed to be in 1..=last, so both indices are valid.
                let low = idx - 1;
                let high = idx;
                let low_value = spacing.at(low);
                let high_value = spacing.at(high);
                if value - low_value < high_value - value {
                    low
                } else {
                    high
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
    use rstest::rstest;

    #[rstest]
    #[case("30")]
    #[case("256")]
    #[case("768")]
    #[case("1024")]
    fn it_works(#[case] name: &str) {
        serde_json::from_str::<Layer>(
            &std::fs::read_to_string(format!("../data/{name}/layer.json")).unwrap(),
        )
        .unwrap();

        let _layer = Layer::from_file(&format!("../data/{name}/layer.json"));
    }
}
