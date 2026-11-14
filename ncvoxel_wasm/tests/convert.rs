//! End-to-end checks: NetCDF -> eslpk -> `shearslice_core::VoxelLayer`, and byte comparison with
//! the Esri-produced reference trees unpacked under `../data/` (git-ignored; tests skip when the
//! files are absent).

use std::collections::HashMap;
use std::path::Path;

use ncvoxel_wasm::reader::Dataset;
use ncvoxel_wasm::{Options, convert};
use rstest::rstest;
use shearslice_core::{Layer, MemoryLoader, VoxelLayer};

const DATA: &str = "../data";

fn read(path: &str) -> Option<Vec<u8>> {
    Path::new(path)
        .exists()
        .then(|| std::fs::read(path).expect("read"))
}

fn convert_file(name: &str) -> Option<(Vec<u8>, HashMap<String, Vec<u8>>)> {
    let nc = read(&format!("{DATA}/2D_Result_{name}.nc"))?;
    let tree = convert(
        &nc,
        &Options {
            name: name.into(),
            ..Options::default()
        },
    )
    .expect("convert");
    Some((nc, tree.files.into_iter().collect()))
}

/// Every voxel of every variable read back through shearslice_core equals the NetCDF value.
#[rstest]
#[case("30")]
#[case("256")]
fn round_trip_through_shearslice_core(#[case] name: &str) {
    let Some((nc, files)) = convert_file(name) else {
        eprintln!("skipping: {DATA}/2D_Result_{name}.nc not present");
        return;
    };
    let layer_json = std::str::from_utf8(&files["layer.json"]).expect("utf8");
    let mut loader = MemoryLoader::new();
    for (path, bytes) in &files {
        loader.insert(path, bytes.clone());
    }
    let layer = VoxelLayer::with_loader(Layer::from_str(layer_json), loader);

    let ds = Dataset::from_bytes(&nc).expect("open");
    let dims: HashMap<_, _> = ds.dims().expect("dims").into_iter().collect();
    let (nz, ny, nx) = (dims["z"], dims["lat"], dims["lon"]);

    let vars: Vec<_> = ds
        .variables()
        .expect("vars")
        .into_iter()
        .filter(|(_, d, _)| d.len() == 4)
        .map(|(n, _, _)| n)
        .collect();
    assert!(!vars.is_empty());
    for (vid, var) in vars.iter().enumerate() {
        let slab = ds.read_slab::<f32>(var, 0, [nz, ny, nx]).expect("slab");
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    assert_eq!(
                        layer.query(x, y, z, vid),
                        Some(slab[(z * ny + y) * nx + x]),
                        "{var} at x={x} y={y} z={z}"
                    );
                }
            }
        }
    }
}

/// Structural JSON diff with a numeric tolerance; returns the first mismatch as `path: a vs b`.
fn json_diff(path: &str, a: &serde_json::Value, b: &serde_json::Value, tol: f64) -> Option<String> {
    use serde_json::Value::{Array, Number, Object};
    match (a, b) {
        (Number(x), Number(y)) => {
            let (x, y) = (x.as_f64()?, y.as_f64()?);
            ((x - y).abs() > tol).then(|| format!("{path}: {x} vs {y}"))
        }
        (Array(x), Array(y)) => {
            if x.len() != y.len() {
                return Some(format!("{path}: length {} vs {}", x.len(), y.len()));
            }
            x.iter()
                .zip(y)
                .enumerate()
                .find_map(|(i, (p, q))| json_diff(&format!("{path}[{i}]"), p, q, tol))
        }
        (Object(x), Object(y)) => {
            let keys: std::collections::BTreeSet<_> = x.keys().chain(y.keys()).collect();
            keys.into_iter().find_map(|k| match (x.get(k), y.get(k)) {
                (Some(p), Some(q)) => json_diff(&format!("{path}.{k}"), p, q, tol),
                (p, q) => Some(format!(
                    "{path}.{k}: present {} vs {}",
                    p.is_some(),
                    q.is_some()
                )),
            })
        }
        _ => (a != b).then(|| format!("{path}: {a} vs {b}")),
    }
}

fn u32_at(b: &[u8], i: usize) -> usize {
    u32::from_le_bytes(b[i..i + 4].try_into().expect("4 bytes")) as usize
}

/// Compare our output with the unpacked Esri reference in `../data/<name>`: chunk headers, the
/// data region of every brick (and its lower apron), the `tree.bin` TREE and TAIL sections, and
/// `index.json`. MID min/max and the x-high over-extent are known to differ (see
/// theloderma `reference/converter-discrepancies.md`).
#[rstest]
#[case("30")]
#[case("256")]
fn matches_esri_reference(#[case] name: &str) {
    let Some((_, files)) = convert_file(name) else {
        eprintln!("skipping: {DATA}/2D_Result_{name}.nc not present");
        return;
    };
    let root = format!("{DATA}/{name}");
    let Some(ref_layer) = read(&format!("{root}/layer.json")) else {
        eprintln!("skipping: {root}/layer.json not present");
        return;
    };
    let ours_layer: serde_json::Value = serde_json::from_slice(&files["layer.json"]).expect("json");
    let theirs: serde_json::Value = serde_json::from_slice(&ref_layer).expect("json");
    assert_eq!(ours_layer["index"], theirs["index"]);
    assert_eq!(
        ours_layer["variables"].as_array().map(Vec::len),
        theirs["variables"].as_array().map(Vec::len)
    );
    if let Some(d) = json_diff(
        "dimensions",
        &ours_layer["volumes"][0]["dimensions"],
        &theirs["volumes"][0]["dimensions"],
        1e-6,
    ) {
        panic!("{d}");
    }
    for k in ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"] {
        let (a, b) = (
            ours_layer["fullExtent"][k].as_f64().expect("f64"),
            theirs["fullExtent"][k].as_f64().expect("f64"),
        );
        // Esri derives a slightly different step for noisy float32 axes (e.g. z step 0.9999990 vs
        // our 1.0 on the 256 set), worth ~4e-5 m at the far edge; 1 mm is far below any visible effect.
        assert!((a - b).abs() < 1e-3, "fullExtent.{k}: {a} vs {b}");
    }

    let dims = &theirs["volumes"][0]["dimensions"];
    let size = |i: usize| dims[i]["size"].as_u64().expect("size") as usize;
    let (nx, ny, nz) = (size(0), size(1), size(2));
    let nvars = theirs["variables"].as_array().expect("variables").len();

    let mut compared = 0;
    for v in 0..nvars {
        let p = format!("variables/{v}");
        let ours_idx: serde_json::Value =
            serde_json::from_slice(&files[&format!("{p}/index.json")]).expect("json");
        let ref_idx: serde_json::Value =
            serde_json::from_slice(&read(&format!("{root}/{p}/index.json")).expect("ref index"))
                .expect("json");
        assert_eq!(ours_idx, ref_idx, "{p}/index.json");

        // tree.bin: TREE (N*48) and TAIL identical; MID (N*72) skipped.
        let ours = &files[&format!("{p}/0/tree.bin")];
        let theirs = read(&format!("{root}/{p}/0/tree.bin")).expect("ref tree");
        assert_eq!(ours.len(), theirs.len(), "{p} tree.bin size");
        let n = u32_at(ours, 0);
        assert_eq!(ours[..48 * n], theirs[..48 * n], "{p} tree.bin TREE");
        assert_eq!(ours[120 * n..], theirs[120 * n..], "{p} tree.bin TAIL");

        // chunks
        let dir = format!("{root}/{p}/0/bundles/0");
        for entry in std::fs::read_dir(&dir).expect("bundles dir") {
            let fname = entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("utf8");
            let Some(stem) = fname.strip_suffix(".bin") else {
                continue;
            };
            let theirs = std::fs::read(format!("{dir}/{fname}")).expect("read");
            let ours = files
                .get(&format!("{p}/0/bundles/0/{fname}"))
                .unwrap_or_else(|| panic!("missing chunk {fname}"));
            assert_eq!(ours.len(), theirs.len(), "{p}/{stem} size");
            assert_eq!(ours[..24], theirs[..24], "{p}/{stem} header");

            let mask = u64::from_le_bytes(ours[16..24].try_into().expect("8"));
            let payload = u32_at(ours, 12);
            let o: Vec<usize> = stem
                .split('-')
                .map(|s| s.parse().expect("origin"))
                .collect();
            let mut k = 0;
            for slot in 0..64usize {
                if mask >> slot & 1 == 0 {
                    continue;
                }
                let base = 24 + k * payload;
                k += 1;
                let (lx, ly, lz) = (slot % 4, slot / 4 % 4, slot / 16);
                let (bx, by, bz) = (o[0] + lx, o[1] + ly, o[2] + lz);
                let vox = |buf: &[u8], z: usize, y: usize, x: usize| {
                    let i = base + ((z * 34 + y) * 34 + x) * 4;
                    buf[i..i + 4].to_vec()
                };
                // padded index p maps to data index bx*32 + p - 1; compare where that is in range
                // (p == 0 is the lower apron, which is data index bx*32 - 1 or the clamped edge).
                for z in 0..34 {
                    for y in 0..34 {
                        for x in 0..34 {
                            let in_data =
                                (bz * 32 + z <= nz) && (by * 32 + y <= ny) && (bx * 32 + x <= nx);
                            if in_data {
                                assert_eq!(
                                    vox(ours, z, y, x),
                                    vox(&theirs, z, y, x),
                                    "{p}/{stem} brick ({bx},{by},{bz}) at z={z} y={y} x={x}"
                                );
                            }
                        }
                    }
                }
            }
            compared += 1;
        }
    }
    assert!(compared > 0);
}
