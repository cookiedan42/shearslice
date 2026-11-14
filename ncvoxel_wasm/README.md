# ncvoxel_wasm

Convert a NetCDF file into an I3S **Voxel layer** in the browser (Rust → wasm). It is a port of the
Python `theloderma` converter (`nc_to_eslpk` + `eslpk_to_slpk`), and its output can be displayed by
an ArcGIS `VoxelLayer`, for example through the local-`.slpk` service-worker flow in `voxel-tester`.

## Input

NetCDF-3 (classic, 64-bit offset, CDF-5) or NetCDF-4 (HDF5), read with the pure-Rust
[`netcdf-reader`](https://crates.io/crates/netcdf-reader) crate (no C libraries).

- Data variables are those with dimensions exactly `(time, z, lat, lon)`, in file order. The order
  becomes the variable id. Types: `float32`, `float64`, `int16`, `int32`, `uint8`, `uint16`.
- Coordinate variables `lon`, `lat`, `z`, `time` must exist. `lat`/`lon` hold projected metres.
- `scale_factor`, `add_offset` and `_FillValue` are honoured. Bricks keep the raw values; stats and
  the tree use decoded values. NaN and the fill value are excluded from stats.
- Spatial reference: WKID override, else `wkid`/`latestWkid`/`epsg_code` on the `crs` variable (then
  globals), else WKT from `spatial_ref`/`crs_wkt`. There is no reprojection.

## API

```ts
import init, { ConvertOptions, netcdf_to_slpk, netcdf_to_files, inspect_netcdf } from "ncvoxel_wasm";

const nc = new Uint8Array(await file.arrayBuffer());
inspect_netcdf(nc);                 // { dimensions, variables: [{ name, dtype, unit, isVoxelData }], spatialReference }

const opts = new ConvertOptions();  // all optional
opts.set_name("my-layer");
opts.set_wkid(3414);                // override the spatial reference
opts.set_shift_x(0); opts.set_shift_y(0); opts.set_shift_z(0);
opts.set_vertical_exaggeration(1);

const slpk  = netcdf_to_slpk(nc, opts);   // Uint8Array: STORE zip of gzipped entries + @specialIndexFileHASH128@
const files = netcdf_to_files(nc, opts);  // Map<path, Uint8Array>: uncompressed eslpk tree
```

`slpk` is what voxel-tester's `buildZipEntryIndex` → IndexedDB → service worker path expects. `files`
holds `layer.json`, `variables/<id>/index.json`, `variables/<id>/stats.json` and, per timestep,
`variables/<id>/<t>/{tree.bin,stats.json,bundles/0/<cx>-<cy>-<cz>.bin}`.

Rust callers use `ncvoxel_wasm::{convert, convert_to_slpk, inspect, Options}`.

## Build

```sh
make wasm-nc          # wasm-pack build -> pkg-nc/
cargo test -p ncvoxel_wasm
```

The tests that need sample data read `../data/` (git-ignored) and skip when it is absent. They check
every voxel against `shearslice_core`'s reader and compare chunk headers, brick data, `tree.bin`
topology and `index.json` with the Esri-produced reference trees.

## Limits

- The whole `.nc` file is held in memory, plus one (variable, timestep) slab at a time. Measured in
  Node: a 206 MB file peaks around 0.9 GB RSS. wasm32 caps linear memory at 4 GB, so files of roughly
  600–800 MB or more (the 1024² sample is 822 MB) will fail to convert in the browser.
- Dimension names are fixed (`time, z, lat, lon`). Only continuous variables are written.
- Known, intentional differences from Esri's producer are listed in theloderma's
  `reference/converter-discrepancies.md` (`sampledCount` rule, mid-level tree min/max, the x-high
  over-extent column). `fullExtent` can differ from Esri's by ~4e-5 m on noisy float32 axes.
