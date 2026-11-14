//! Thin wrapper over `netcdf-reader` exposing just what the converter needs.

use netcdf_reader::{NcAttrValue, NcFile, NcReadable, NcSliceInfo, NcSliceInfoElem, NcType};

use crate::{Error, Result};

/// Raw storage types a data variable may have. These are the types bricks can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DType {
    F32,
    F64,
    I16,
    I32,
    U8,
    U16,
}

impl DType {
    pub fn from_nc(t: &NcType) -> Result<Self> {
        Ok(match t {
            NcType::Float => Self::F32,
            NcType::Double => Self::F64,
            NcType::Short => Self::I16,
            NcType::Int => Self::I32,
            NcType::UByte => Self::U8,
            NcType::UShort => Self::U16,
            other => return Err(Error::new(format!("unsupported variable type {other:?}"))),
        })
    }

    /// `renderingFormat.type` string used in `layer.json`.
    pub const fn format_name(self) -> &'static str {
        match self {
            Self::F32 => "Float32",
            Self::F64 => "Float64",
            Self::I16 => "Int16",
            Self::I32 => "Int32",
            Self::U8 => "UInt8",
            Self::U16 => "UInt16",
        }
    }
}

/// A raw voxel value type: readable from NetCDF and storable little-endian in a brick.
pub trait Sample: NcReadable + Copy {
    const ZERO: Self;
    fn to_f64(self) -> f64;
    fn push_le(self, out: &mut Vec<u8>);
}

macro_rules! impl_sample {
    ($($t:ty),*) => {$(
        impl Sample for $t {
            const ZERO: Self = 0 as $t;
            fn to_f64(self) -> f64 { f64::from(self) }
            fn push_le(self, out: &mut Vec<u8>) { out.extend_from_slice(&self.to_le_bytes()); }
        }
    )*};
}
impl_sample!(f32, f64, i16, i32, u8, u16);

pub struct Dataset {
    file: NcFile,
}

impl Dataset {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(Self {
            file: NcFile::from_bytes(bytes)?,
        })
    }

    pub fn dim_len(&self, name: &str) -> Option<usize> {
        let d = self.file.dimension(name).ok()?;
        usize::try_from(d.size).ok()
    }

    pub fn dims(&self) -> Result<Vec<(String, usize)>> {
        Ok(self
            .file
            .dimensions()?
            .iter()
            .map(|d| {
                (
                    d.name.clone(),
                    usize::try_from(d.size).unwrap_or(usize::MAX),
                )
            })
            .collect())
    }

    /// All variables as `(name, dimension names, storage type)`, in file order.
    pub fn variables(&self) -> Result<Vec<(String, Vec<String>, NcType)>> {
        Ok(self
            .file
            .variables()?
            .iter()
            .map(|v| {
                (
                    v.name().to_string(),
                    v.dimensions().iter().map(|d| d.name.clone()).collect(),
                    v.dtype().clone(),
                )
            })
            .collect())
    }

    /// Attribute of variable `var`, or a global attribute when `var` is `None`.
    pub fn attr(&self, var: Option<&str>, name: &str) -> Option<NcAttrValue> {
        match var {
            Some(v) => self
                .file
                .variable(v)
                .ok()?
                .attribute(name)
                .map(|a| a.value.clone()),
            None => self
                .file
                .global_attribute(name)
                .ok()
                .map(|a| a.value.clone()),
        }
    }

    pub fn attr_string(&self, var: Option<&str>, name: &str) -> Option<String> {
        self.attr(var, name)?.as_string().filter(|s| !s.is_empty())
    }

    pub fn attr_f64(&self, var: Option<&str>, name: &str) -> Option<f64> {
        self.attr(var, name)?.as_f64()
    }

    /// Whole variable promoted to f64 (used for the 1-D coordinate variables).
    pub fn read_f64(&self, var: &str) -> Result<Vec<f64>> {
        Ok(self
            .file
            .read_variable_as_f64(var)?
            .iter()
            .copied()
            .collect())
    }

    /// One timestep, shape `(nz, ny, nx)`, of a `(time, z, lat, lon)` variable.
    pub fn read_slab<T: Sample>(&self, var: &str, t: usize, shape: [usize; 3]) -> Result<Vec<T>> {
        let mut sel = NcSliceInfo::all(4);
        sel.selections[0] = NcSliceInfoElem::Index(t as u64);
        let arr = self.file.read_variable_slice::<T>(var, &sel)?;
        if arr.shape() != shape {
            return Err(Error::new(format!(
                "variable {var}: slab shape {:?} != expected {shape:?}",
                arr.shape()
            )));
        }
        if arr.is_standard_layout() {
            Ok(arr.into_raw_vec_and_offset().0)
        } else {
            Ok(arr.iter().copied().collect())
        }
    }
}
