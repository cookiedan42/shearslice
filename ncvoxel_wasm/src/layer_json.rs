//! `layer.json` builder (port of theloderma `layer_json.py`).

use serde_json::{Map, Value, json};

use crate::color_ramp::RGBA;
use crate::reader::DType;
use crate::stats::Stats;

/// One coordinate axis (`lon`, `lat`, `z` or `time`).
pub struct Axis {
    pub values: Vec<f64>,
    /// Source was stored as float32. Such axes carry quantisation noise, so "uniform" is judged
    /// with a relative tolerance (see [`F32_REL_TOL`]) instead of bit-exactly.
    pub is_f32: bool,
    pub label: Option<String>,
    pub unit: Option<String>,
}

/// One data variable.
pub struct VarInfo {
    pub name: String,
    pub unit: String,
    pub dtype: DType,
    pub scale: Option<f64>,
    pub offset: Option<f64>,
    /// Raw no-data sentinel; `true` in `.1` when it was stored as a float.
    pub no_data: Option<(f64, bool)>,
}

pub struct Coords {
    pub lon: Axis,
    pub lat: Axis,
    pub z: Axis,
    pub time: Axis,
}

/// Largest step deviation, relative to the step, still treated as uniform on a float32 axis.
/// Float32 rounding of a ~1e4 m coordinate is ~4e-3 m (so a 0.9 m step jitters by ~4e-3 relative:
/// irregular), while on a ~10 m coordinate it is ~4e-6 (jitter ~4e-6 relative: regular). The
/// Esri producer's cut-off lies between; this is the geometric middle.
const F32_REL_TOL: f64 = 1e-4;
const F64_REL_TOL: f64 = 1e-9;

/// The step when the axis is uniform, else `None`. Exactly uniform axes return their exact
/// difference; noisy ones return `(last - first) / (n - 1)`.
fn uniform_step(a: &Axis) -> Option<f64> {
    let v = &a.values;
    if v.len() <= 1 {
        return None;
    }
    let d0 = v[1] - v[0];
    if v.windows(2).all(|w| w[1] - w[0] == d0) {
        return Some(d0);
    }
    let step = (v[v.len() - 1] - v[0]) / (v.len() - 1) as f64;
    let tol = step.abs() * if a.is_f32 { F32_REL_TOL } else { F64_REL_TOL };
    v.windows(2)
        .all(|w| (w[1] - w[0] - step).abs() <= tol)
        .then_some(step)
}

fn min_max(v: &[f64]) -> (f64, f64) {
    v.iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &x| {
            (lo.min(x), hi.max(x))
        })
}

/// Cell-edge extent: on a uniform axis both ends are shifted by `-0.5*step`.
fn edge_extent(a: &Axis) -> (f64, f64) {
    let (lo, hi) = min_max(&a.values);
    uniform_step(a).map_or((lo, hi), |step| {
        // Derived from the regular spacing (`offset + (n-1)*scale`), not the last sample, so the
        // extent agrees with `regularSpacing` even when a float32 axis carries rounding noise.
        let first = a.values[0] - 0.5 * step;
        let last = first + (a.values.len() - 1) as f64 * step;
        (first.min(last), first.max(last))
    })
}

fn dimension(name: &str, a: &Axis) -> Value {
    let label = a.label.clone().unwrap_or_else(|| {
        match name {
            "lon" => "x coordinate of projection",
            "lat" => "y coordinate of projection",
            "z" => "height above surface",
            _ => "time",
        }
        .to_string()
    });
    let mut out = Map::new();
    out.insert("name".into(), json!(name));
    out.insert("label".into(), json!(label));
    out.insert("unit".into(), json!(a.unit.clone().unwrap_or_default()));
    out.insert("size".into(), json!(a.values.len()));
    match uniform_step(a) {
        Some(step) if a.values.len() > 1 => {
            let mut reg = Map::new();
            if step != 1.0 {
                reg.insert("scale".into(), json!(step));
            }
            // Cell-edge convention, consistent with the half-step shift in `fullExtent`.
            reg.insert("offset".into(), json!(a.values[0] - 0.5 * step));
            out.insert("regularSpacing".into(), Value::Object(reg));
        }
        _ => {
            out.insert("irregularSpacing".into(), json!({ "values": a.values }));
        }
    }
    let quantity = match name {
        "lat" => Some("horizontal-coordinate"),
        "z" => Some("vertical-coordinate"),
        "time" => Some("time"),
        _ => None,
    };
    if let Some(q) = quantity {
        out.insert("quantity".into(), json!(q));
    }
    Value::Object(out)
}

fn format_block(v: &VarInfo) -> Value {
    let mut out = Map::new();
    out.insert("type".into(), json!(v.dtype.format_name()));
    out.insert("continuity".into(), json!("continuous"));
    if let Some(s) = v.scale {
        out.insert("scale".into(), json!(s));
    }
    if let Some(o) = v.offset {
        out.insert("offset".into(), json!(o));
    }
    if let Some((nd, is_float)) = v.no_data {
        out.insert(
            "noData".into(),
            if is_float {
                json!(nd)
            } else {
                json!(nd as i64)
            },
        );
        out.insert("hasNoData".into(), json!(true));
    }
    Value::Object(out)
}

/// `[p10, p90]` of the CDF built from the 256-bin distribution (the Esri `stretchRange` rule).
pub fn stretch_range(s: &Stats) -> (f64, f64) {
    let [mn, mx] = s.data_range;
    let [d0, d1] = s.distribution_range;
    let len = s.distribution.len();
    if len < 2 || d0 == d1 {
        return (mn, mx);
    }
    let mut cdf = Vec::with_capacity(len + 1);
    cdf.push(0.0);
    let mut acc = 0.0;
    for &p in &s.distribution {
        acc += p;
        cdf.push(acc);
    }
    if acc <= 0.0 {
        return (mn, mx);
    }
    for c in &mut cdf {
        *c /= acc;
    }
    // edges = linspace(d0, d1, len + 1); np.interp(q, cdf, edges)
    let edge = |i: usize| d0 + (d1 - d0) * i as f64 / len as f64;
    let interp = |q: f64| {
        let j = cdf.partition_point(|&x| x <= q).saturating_sub(1);
        if j >= len {
            return edge(len);
        }
        let (x0, x1) = (cdf[j], cdf[j + 1]);
        if x1 == x0 {
            edge(j)
        } else {
            edge(j) + (q - x0) * (edge(j + 1) - edge(j)) / (x1 - x0)
        }
    };
    (interp(0.10), interp(0.90))
}

pub fn build(
    name: &str,
    spatial_reference: &Value,
    coords: &Coords,
    variables: &[VarInfo],
    var_stats: &[Stats],
    vertical_exaggeration: f64,
) -> Value {
    let (xmin, xmax) = edge_extent(&coords.lon);
    let (ymin, ymax) = edge_extent(&coords.lat);
    let (zmin, zmax) = edge_extent(&coords.z);

    let colour_stops: Vec<Value> = RGBA
        .iter()
        .enumerate()
        .map(|(i, c)| json!({ "color": c, "position": i as f64 / (RGBA.len() - 1) as f64 }))
        .collect();

    let variable_styles: Vec<Value> = var_stats
        .iter()
        .enumerate()
        .map(|(vid, s)| {
            let (lo, hi) = stretch_range(s);
            json!({
                "variableId": vid,
                "transferFunction": {
                    "interpolation": "linear",
                    "stretchRange": [lo, hi],
                    "colorStops": colour_stops,
                }
            })
        })
        .collect();

    json!({
        "layerType": "Voxel",
        "version": "3.0",
        "name": name,
        "spatialReference": spatial_reference,
        "fullExtent": {
            "spatialReference": spatial_reference,
            "xmin": xmin, "xmax": xmax,
            "ymin": ymin, "ymax": ymax,
            "zmin": zmin, "zmax": zmax,
        },
        "volumes": [{
            "id": 0,
            "dimensions": [
                dimension("lon", &coords.lon),
                dimension("lat", &coords.lat),
                dimension("z", &coords.z),
                dimension("time", &coords.time),
            ],
        }],
        "variables": variables.iter().enumerate().map(|(id, v)| json!({
            "id": id,
            "name": v.name,
            "description": v.name,
            "unit": v.unit,
            "originalFormat": format_block(v),
            "renderingFormat": format_block(v),
        })).collect::<Vec<_>>(),
        "index": {
            "brickSize": [32, 32, 32],
            "nodeSize": [4, 4, 4],
            "apronWidth": 1,
            "maxLodLevel": 0,
        },
        "style": {
            "volumeStyles": [{ "volumeId": 0, "verticalExaggeration": vertical_exaggeration }],
            "currentVariableId": 0,
            "variableStyles": variable_styles,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis(values: &[f64], is_f32: bool) -> Axis {
        Axis {
            values: values.to_vec(),
            is_f32,
            label: None,
            unit: Some("metre".into()),
        }
    }

    #[test]
    fn uniform_axis_is_regular_with_half_step_extent() {
        let a = axis(&[10.0, 12.0, 14.0, 16.0], false);
        assert_eq!(uniform_step(&a), Some(2.0));
        assert_eq!(edge_extent(&a), (9.0, 15.0));
        let d = dimension("lat", &a);
        assert_eq!(d["regularSpacing"], json!({ "scale": 2.0, "offset": 9.0 }));
        assert_eq!(d["quantity"], "horizontal-coordinate");
    }

    #[test]
    fn unit_step_drops_scale() {
        let d = dimension("z", &axis(&[0.0, 1.0, 2.0], false));
        assert_eq!(d["regularSpacing"], json!({ "offset": -0.5 }));
    }

    #[test]
    fn single_value_and_ragged_axes_are_irregular() {
        let t = dimension("time", &axis(&[5.0], false));
        assert_eq!(t["irregularSpacing"], json!({ "values": [5.0] }));
        let ragged = axis(&[0.0, 1.0, 3.0], false);
        assert_eq!(edge_extent(&ragged), (0.0, 3.0));
        assert!(dimension("lon", &ragged).get("quantity").is_none());
    }

    #[test]
    fn f32_axis_tolerates_quantisation_noise_only_when_small() {
        // ~10 m coordinates with 1 m step: f32 jitter ~4e-6 => regular.
        let z: Vec<f64> = (0..49)
            .map(|i| f64::from((-8.595_633f32 + i as f32) * 1.0))
            .collect();
        let a = axis(&z, true);
        assert!(uniform_step(&a).is_some_and(|s| (s - 1.0).abs() < 1e-5));
        // ~3e4 m coordinates with ~0.92 m step: f32 jitter ~4e-3 => irregular.
        let lon: Vec<f64> = (0..40)
            .map(|i| f64::from((33_645.4f64 + 0.9217 * f64::from(i)) as f32))
            .collect();
        assert_eq!(uniform_step(&axis(&lon, true)), None);
        // the same noise on an f64 axis is irregular
        assert_eq!(uniform_step(&axis(&z, false)), None);
    }

    #[test]
    fn stretch_range_of_uniform_histogram() {
        let s = Stats {
            data_range: [0.0, 100.0],
            distribution_range: [0.0, 100.0],
            distribution: vec![1.0 / 256.0; 256],
            mean: 50.0,
            sampled_sum: 0.0,
            sampled_count: 0,
            sampled_sum_sqr: 0.0,
            std_dev: None,
        };
        let (lo, hi) = stretch_range(&s);
        assert!(
            (lo - 10.0).abs() < 1e-9 && (hi - 90.0).abs() < 1e-9,
            "{lo} {hi}"
        );
    }

    #[test]
    fn format_block_carries_scale_and_fill() {
        let v = VarInfo {
            name: "w".into(),
            unit: "m/s".into(),
            dtype: DType::I16,
            scale: Some(0.5),
            offset: Some(1.0),
            no_data: Some((-32768.0, false)),
        };
        assert_eq!(
            format_block(&v),
            json!({ "type": "Int16", "continuity": "continuous", "scale": 0.5, "offset": 1.0,
                    "noData": -32768, "hasNoData": true })
        );
    }
}
