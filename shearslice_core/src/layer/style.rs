//! Structs for dealing with the style section of a variable

use serde::{Deserialize, Serialize};

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

/// Transfer function defines how color is applied to the voxel layer
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
    #[serde(default)]
    vertical_exaggeration: f32,
}
impl VolumeStyle {
    pub(crate) fn vertical_exaggeration(&self) -> f32 {
        self.vertical_exaggeration
    }
}
