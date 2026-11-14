//! NetCDF → I3S Voxel layer converter (Rust port of the Python `theloderma` package).
//!
//! Input is a NetCDF-3/NetCDF-4 file whose data variables share the dimensions
//! `(time, z, lat, lon)`. Output is an *eslpk* file tree (`layer.json`, `variables/<id>/...`),
//! either as a plain path→bytes list ([`convert`]) or packed as a `.slpk` archive
//! ([`convert_to_slpk`]) that can be served to an ArcGIS `VoxelLayer`.

pub mod bricks;
pub mod color_ramp;
pub mod layer_json;
pub mod reader;
pub mod slpk;
pub mod stats;
pub mod tree;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

use std::fmt;

use serde_json::{Value, json};

use layer_json::{Axis, Coords, VarInfo};
use reader::{DType, Dataset, Sample};
use stats::Stats;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(String);

impl Error {
    pub fn new(msg: impl Into<String>) -> Self {
        Self(msg.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<netcdf_reader::Error> for Error {
    fn from(e: netcdf_reader::Error) -> Self {
        Self(format!("netcdf: {e}"))
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self(format!("io: {e}"))
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self(format!("json: {e}"))
    }
}

pub type Result<T> = std::result::Result<T, Error>;

const DATA_VAR_DIMS: [&str; 4] = ["time", "z", "lat", "lon"];

#[derive(Clone, Debug)]
pub struct Options {
    /// Layer name written to `layer.json`.
    pub name: String,
    /// Added to every x (`lon`) / y (`lat`) / z coordinate.
    pub shift_x: f64,
    pub shift_y: f64,
    pub shift_z: f64,
    /// Overrides any WKID found in the file.
    pub wkid: Option<u32>,
    /// Initial z multiplier (`style.volumeStyles[0].verticalExaggeration`).
    pub vertical_exaggeration: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            name: "voxel".to_string(),
            shift_x: 0.0,
            shift_y: 0.0,
            shift_z: 0.0,
            wkid: None,
            vertical_exaggeration: 1.0,
        }
    }
}

/// An eslpk tree: uncompressed files keyed by relative path.
#[derive(Default)]
pub struct Eslpk {
    pub files: Vec<(String, Vec<u8>)>,
}

impl Eslpk {
    /// Pack as a `.slpk` (STORE zip of gzipped entries plus the hash index).
    pub fn to_slpk(&self) -> Result<Vec<u8>> {
        slpk::pack(&self.files)
    }
}

/// Convert to an in-memory eslpk tree.
pub fn convert(nc: &[u8], opts: &Options) -> Result<Eslpk> {
    let mut files = Vec::new();
    convert_with(nc, opts, &mut |path, data| {
        files.push((path, data));
        Ok(())
    })?;
    Ok(Eslpk { files })
}

/// Convert straight to `.slpk` bytes, gzipping each file as it is produced so the uncompressed
/// tree never exists in memory at once.
pub fn convert_to_slpk(nc: &[u8], opts: &Options) -> Result<Vec<u8>> {
    let mut w = slpk::SlpkWriter::new();
    convert_with(nc, opts, &mut |path, data| w.add(&path, &data))?;
    w.finish()
}

/// Sink for produced files: `(relative path, uncompressed bytes)`.
pub type Sink<'a> = &'a mut dyn FnMut(String, Vec<u8>) -> Result<()>;

pub fn convert_with(nc: &[u8], opts: &Options, sink: Sink) -> Result<()> {
    let ds = Dataset::from_bytes(nc)?;

    let size = |name: &str| {
        ds.dim_len(name)
            .ok_or_else(|| Error::new(format!("missing dimension `{name}`")))
    };
    let (nt, nz, ny, nx) = (size("time")?, size("z")?, size("lat")?, size("lon")?);
    let shape = [nz, ny, nx];

    let coords = read_coords(&ds, opts)?;
    let spatial_reference = read_spatial_reference(&ds, opts.wkid)?;

    let data_vars: Vec<_> = ds
        .variables()?
        .into_iter()
        .filter(|(_, dims, _)| dims.iter().map(String::as_str).eq(DATA_VAR_DIMS))
        .collect();
    if data_vars.is_empty() {
        return Err(Error::new(format!(
            "no data variables with dimensions {DATA_VAR_DIMS:?} found in NetCDF"
        )));
    }

    let mut infos = Vec::new();
    let mut var_stats = Vec::new();
    for (vid, (name, _, ty)) in data_vars.iter().enumerate() {
        let dtype = DType::from_nc(ty)?;
        let params = VarParams::read(&ds, name);
        let ctx = VarCtx {
            ds: &ds,
            name,
            vid,
            nt,
            shape,
            params: &params,
        };
        let agg = match dtype {
            DType::F32 => convert_variable::<f32>(&ctx, sink)?,
            DType::F64 => convert_variable::<f64>(&ctx, sink)?,
            DType::I16 => convert_variable::<i16>(&ctx, sink)?,
            DType::I32 => convert_variable::<i32>(&ctx, sink)?,
            DType::U8 => convert_variable::<u8>(&ctx, sink)?,
            DType::U16 => convert_variable::<u16>(&ctx, sink)?,
        };
        infos.push(VarInfo {
            name: name.clone(),
            unit: ds.attr_string(Some(name), "units").unwrap_or_default(),
            dtype,
            scale: params.scale,
            offset: params.offset,
            no_data: params.raw_fill,
        });
        var_stats.push(agg);
    }

    let layer = layer_json::build(
        &opts.name,
        &spatial_reference,
        &coords,
        &infos,
        &var_stats,
        opts.vertical_exaggeration,
    );
    sink("layer.json".into(), serde_json::to_vec_pretty(&layer)?)
}

/// `scale_factor` / `add_offset` / `_FillValue` of a data variable.
struct VarParams {
    scale: Option<f64>,
    offset: Option<f64>,
    /// Raw fill value, and whether it was stored as a float.
    raw_fill: Option<(f64, bool)>,
}

impl VarParams {
    fn read(ds: &Dataset, var: &str) -> Self {
        let raw_fill = ds.attr(Some(var), "_FillValue").and_then(|a| {
            let is_float = matches!(
                a,
                netcdf_reader::NcAttrValue::Floats(_) | netcdf_reader::NcAttrValue::Doubles(_)
            );
            a.as_f64().map(|v| (v, is_float))
        });
        Self {
            scale: ds.attr_f64(Some(var), "scale_factor"),
            offset: ds.attr_f64(Some(var), "add_offset"),
            raw_fill,
        }
    }

    fn packed(&self) -> bool {
        self.scale.is_some() || self.offset.is_some()
    }

    /// `decoded = raw*scale + offset` (identity when neither attribute exists).
    fn decode(&self, raw: f64) -> f64 {
        if self.packed() {
            raw * self.scale.unwrap_or(1.0) + self.offset.unwrap_or(0.0)
        } else {
            raw
        }
    }

    fn decoded_fill(&self) -> Option<f64> {
        self.raw_fill.map(|(f, _)| self.decode(f))
    }
}

struct VarCtx<'a> {
    ds: &'a Dataset,
    name: &'a str,
    vid: usize,
    nt: usize,
    shape: [usize; 3],
    params: &'a VarParams,
}

/// Emit `variables/<vid>/...` and return the variable's aggregate stats.
fn convert_variable<T: Sample>(ctx: &VarCtx, sink: Sink) -> Result<Stats> {
    let VarCtx {
        ds,
        name,
        vid,
        nt,
        shape,
        params,
    } = *ctx;
    let [nz, ny, nx] = shape;
    let fill = params.decoded_fill();
    // Decoded value of a voxel, or None for NaN / the fill sentinel.
    let valid = |v: T| {
        let d = params.decode(v.to_f64());
        (!d.is_nan() && Some(d) != fill).then_some(d)
    };

    let base = format!("variables/{vid}");
    let mut per_ts = Vec::with_capacity(nt);
    let mut trees = Vec::with_capacity(nt);

    for t in 0..nt {
        let slab: Vec<T> = ds.read_slab(name, t, shape)?;
        let ts = format!("{base}/{t}");

        bricks::pack_chunks(&slab, shape, |chunk| {
            sink(format!("{ts}/bundles/0/{}", chunk.file_name()), chunk.bytes)
        })?;

        let visit = |f: &mut dyn FnMut(f64)| slab.iter().filter_map(|&v| valid(v)).for_each(f);
        let st = stats::compute(&visit, shape);

        // Leaves with no valid voxel take the slab's overall range.
        let fallback = if st.sampled_count == 0 {
            (0.0, 0.0)
        } else {
            (st.data_range[0], st.data_range[1])
        };
        let leaf = bricks::leaf_minmax(&slab, shape, valid);
        let nodes = tree::build_tree(bricks::grid_shape(shape), &leaf, fallback);
        sink(format!("{ts}/tree.bin"), tree::encode_tree_bin(&nodes))?;
        let c = tree::counts(&nodes);
        trees.push(json!({
            "treeNodeCount": c.node_count,
            "treeChildrenListCount": c.children_list_count,
            "leafBrickCount": c.leaf_brick_count,
        }));

        sink(format!("{ts}/stats.json"), serde_json::to_vec_pretty(&st)?)?;
        per_ts.push(st);
    }

    let index = json!({ "volumeSize": [nx, ny, nz], "trees": trees });
    sink(
        format!("{base}/index.json"),
        serde_json::to_vec_pretty(&index)?,
    )?;
    let agg = stats::aggregate(&per_ts);
    sink(
        format!("{base}/stats.json"),
        serde_json::to_vec_pretty(&agg)?,
    )?;
    Ok(agg)
}

fn read_coords(ds: &Dataset, opts: &Options) -> Result<Coords> {
    let axis = |name: &str, shift: f64| -> Result<Axis> {
        let ty = ds
            .variables()?
            .into_iter()
            .find(|(n, _, _)| n == name)
            .map(|(_, _, t)| t)
            .ok_or_else(|| Error::new(format!("missing coordinate variable `{name}`")))?;
        let mut values = ds.read_f64(name)?;
        let shifted = shift != 0.0;
        for v in &mut values {
            *v += shift;
        }
        Ok(Axis {
            values,
            // A shifted axis is promoted to f64 first, which keeps a uniform step uniform.
            is_f32: matches!(ty, netcdf_reader::NcType::Float) && !shifted,
            label: ds.attr_string(Some(name), "long_name"),
            unit: ds.attr_string(Some(name), "units"),
        })
    };
    Ok(Coords {
        lon: axis("lon", opts.shift_x)?,
        lat: axis("lat", opts.shift_y)?,
        z: axis("z", opts.shift_z)?,
        time: axis("time", 0.0)?,
    })
}

/// WKID (override, then `crs` variable, then globals) or else WKT.
fn read_spatial_reference(ds: &Dataset, wkid_override: Option<u32>) -> Result<Value> {
    const WKID_ATTRS: [&str; 3] = ["wkid", "latestWkid", "epsg_code"];
    let crs = ds
        .variables()?
        .iter()
        .any(|(n, _, _)| n == "crs")
        .then_some("crs");

    let find_wkid = |scope: Option<&str>| WKID_ATTRS.iter().find_map(|a| ds.attr_f64(scope, a));
    let wkid = wkid_override
        .map(f64::from)
        .or_else(|| crs.and_then(|c| find_wkid(Some(c))))
        .or_else(|| find_wkid(None));
    if let Some(w) = wkid {
        let w = w as u32;
        return Ok(json!({ "wkid": w, "latestWkid": w }));
    }

    let wkt = crs
        .and_then(|c| {
            ds.attr_string(Some(c), "spatial_ref")
                .or_else(|| ds.attr_string(Some(c), "crs_wkt"))
        })
        .or_else(|| ds.attr_string(None, "spatial_ref"))
        .or_else(|| ds.attr_string(None, "crs_wkt"));
    wkt.map(|w| json!({ "wkt": w })).ok_or_else(|| {
        Error::new(
            "no spatial reference found in NetCDF: expected `crs:wkid`, `crs:epsg_code`, \
             `crs:spatial_ref`, or a wkid override",
        )
    })
}

/// Light-weight description of a NetCDF file, so a UI can check it before converting.
pub fn inspect(nc: &[u8]) -> Result<Value> {
    let ds = Dataset::from_bytes(nc)?;
    let dims: serde_json::Map<String, Value> =
        ds.dims()?.into_iter().map(|(n, s)| (n, json!(s))).collect();
    let variables: Vec<Value> = ds
        .variables()?
        .into_iter()
        .map(|(name, dims, ty)| {
            json!({
                "name": name,
                "dimensions": dims,
                "dtype": format!("{ty:?}"),
                "unit": ds.attr_string(Some(&name), "units"),
                "isVoxelData": dims.iter().map(String::as_str).eq(DATA_VAR_DIMS),
            })
        })
        .collect();
    let spatial_reference = read_spatial_reference(&ds, None).ok();
    Ok(json!({ "dimensions": dims, "variables": variables, "spatialReference": spatial_reference }))
}
