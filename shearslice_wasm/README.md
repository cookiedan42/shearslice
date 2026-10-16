# @dplplugin/shearslice_wasm

JavaScript/TypeScript bindings (via `wasm-bindgen`) for **shearslice**: a Rust
library that reads an Esri **I3S Voxel Layer** SLPK and lets you slice the 3D
scalar field it contains along an elevation plane (or a per-column
"terrain-following" surface), rendering the slice as a colored PNG using the
color ramp embedded in the SLPK itself.

This package is the browser-facing half of the project. It does not read
files or fetch URLs itself — your JS code fetches the bytes (from a SceneServer,
a local unpacked SLPK served statically, or anywhere else) and hands them to
this package to decode and query.

> Not published to npm. Build it yourself (see [Building](#building)) and
> consume it from the local `pkg/` output, a `file:` dependency, or your own
> registry.

## Install

```sh
npm install file:../path/to/pkg
# or copy pkg/ into your project and import it directly
```

```js
import init, { VoxelData, get_paths } from "@dplplugin/shearslice_wasm";

await init(); // loads and instantiates the .wasm binary
```

(Exact init call depends on the `--target` the package was built with — see
[Building](#building).)

## Core concepts

- **`layer.json`** — the SLPK's metadata document (dimensions, extent,
  variables, color ramps). You fetch this as text and pass it in; this
  package never does its own HTTP.
- **Variable / `layer_id`** — a voxel layer can hold several scalar
  variables (e.g. `WindSpeed`, `Wind_x`, `T`, `p`). Each is identified by a
  numeric `id` from `layer.json`'s `variables` array — **not** always `0`.
  Check `layer.json` before assuming an id; see
  [Picking the right `layer_id`](#picking-the-right-layer_id) below.
- **Node bundles** — the SLPK's binary payload is split into per-node `.bin`
  files (one per spatial tile, per variable). You fetch only the bundles a
  query actually touches; `get_paths`/`VoxelQuery.plan` tell you which ones
  those are.
- **No-data** — a variable may declare a fill ("no-data") value. Queries mask
  it out rather than returning the raw sentinel — see
  [No-data values](#no-data-values).

## API reference

### Module init

- `start()` — runs automatically on module load (`#[wasm_bindgen(start)]`).
  Installs a panic hook (so Rust panics surface as real JS errors in the
  console instead of a silent abort) and a logger.

### `get_paths(url, layerJson, layerId): string[]`

Given a SceneServer base URL (e.g.
`"https://.../rest/services/f773/SceneServer"`), the `layer.json` text, and a
variable id, returns every bundle URL needed to cover the *entire* volume for
that variable:

```
"{url}/variables/{layerId}/0/bundles/0/{node_x}-{node_y}-{node_z}"
```

Fetch each of these (as `ArrayBuffer`/`Uint8Array`) before constructing a
`VoxelData` with `make_layer` — see [Flow A](#flow-a-full-volume--pngs) below.

### `VoxelData` — full-volume, PNG-rendering queries

```ts
class VoxelData {
  static make_layer(layerJson: string, values: Uint8Array[], layerId: number): VoxelData;
  get_corners(): Float64Array;            // [xmin, xmax, ymin, ymax, zmin, zmax]
  get_xy(): PointXy[];                    // full x × y coordinate grid
  query_plane(z: number): Uint8Array;     // PNG bytes, fixed Z-index slice
  query_arr16(arr16: Uint16Array, z: number): Uint8Array; // PNG, per-column Z offset
  query_arr_geometry(points: PointXyz[], layerId: number): Uint8Array; // PNG, explicit world points
  query_geometry(x: number, y: number, z: number, layerId: number): number | undefined;
}
```

- **`make_layer(layerJson, values, layerId)`** — the constructor. `values`
  must be the bundle bytes for `layerId` fetched via `get_paths`, **in the
  same order `get_paths` produced them** (`values[i]` corresponds to
  `get_paths(...)[i]`). Every bundle for that variable must be present — this
  is the "eagerly load the whole volume into memory" path, suited to small
  layers or a webworker you're fine holding resident.
- **`get_corners()`** — the layer's world-coordinate extent, for
  georeferencing the rendered PNG in a map/viewer.
- **`get_xy()`** — the full cross-product of the x and y dimension coordinate
  arrays, as `PointXy[]`.
- **`query_plane(z)`** — a flat, fixed-Z horizontal slice, clamped into range,
  rendered as a PNG using the variable's own color ramp
  (`layer.json`'s `style.variableStyles`) and stretch range.
- **`query_arr16(arr16, z)`** — like `query_plane`, but `arr16[i]` is a
  per-column Z *offset* from the base `z`, letting you trace an arbitrary
  (e.g. terrain-following) surface instead of a flat plane. `arr16` is
  flattened row-major over the volume's x/y grid.
- **`query_arr_geometry(points, layerId)`** — same PNG pipeline, but built
  from an explicit list of world-coordinate `PointXyz`s rather than a grid
  slice; the only `VoxelData` PNG method that takes `layerId` as an argument
  (see the gotcha below).
- **`query_geometry(x, y, z, layerId)`** — a single scalar value at a
  world-coordinate point, no image encoding, no clamping/normalization (raw
  variable units). Returns `undefined` if that voxel holds the variable's
  declared no-data value.

**⚠️ `query_plane` and `query_arr16` only ever read variable id `0`**,
regardless of the `layerId` passed to `make_layer`. If you constructed
`VoxelData` with a non-zero `layerId` (anything other than the layer's first
variable), calling `query_plane`/`query_arr16` on it will throw — the bins it
preloaded are keyed by your `layerId`, but these two methods look up variable
`0`'s bins internally. Use `query_geometry`/`query_arr_geometry` (which both
take an explicit `layerId`) for any variable other than `0`, or use
`VoxelQuery` below.

PNG output is sized to the volume's actual x,y grid dimensions (from the
layer's metadata), not a fixed resolution.

### `VoxelQuery` — memory-lean, per-point batch queries

For large volumes where loading every bundle up front (`VoxelData.make_layer`)
isn't practical, `VoxelQuery` lets you query an arbitrary batch of points
while only ever holding the specific node bundles those points touch in
memory — each bundle is preloaded, queried, and evicted immediately.

```ts
class VoxelQuery {
  constructor(layerJson: string, layerId: number);
  plan(xs: Float32Array, ys: Float32Array, zs: Float32Array): BundlePlan;
  query_bundle(
    nodeX: number, nodeY: number, nodeZ: number,
    bytes: Uint8Array,
    xs: Float32Array, ys: Float32Array, zs: Float32Array,
  ): Float32Array;
}

class BundlePlan {
  readonly bundle_ids: string[];    // "{node_x}-{node_y}-{node_z}" per bundle
  readonly assignments: Uint32Array; // index into bundle_ids per input point, or NO_DATA (0xFFFFFFFF) if out of extent
}
```

Usage: call `plan(xs, ys, zs)` once for your whole batch of points (in the
layer's projected spatial reference) to get the minimal set of bundles that
cover them. For each `bundle_ids[i]`, fetch that bundle's bytes
(`variables/{layerId}/0/bundles/0/{bundle_ids[i]}`), then call
`query_bundle(nodeX, nodeY, nodeZ, bytes, xs, ys, zs)` with **only the subset
of points assigned to that bundle** (`assignments[p] === i`). A point whose
voxel holds the variable's no-data value comes back as `NaN` in the result
array — test with `Number.isNaN(...)`; a point outside the volume's extent is
never assigned a bundle at all (`assignments[p] === 0xFFFFFFFF`) and should be
skipped before calling `query_bundle`.

### `PointXy` / `PointXyz`

Plain coordinate structs:

```ts
class PointXy  { x: number; y: number; static new(x, y): PointXy; }
class PointXyz { x: number; y: number; z: number; static new(x, y, z): PointXyz; }
```

Used as `get_xy()`'s return element type and `query_arr_geometry`'s input.

## No-data values

A variable can declare a fill/no-data value in `layer.json` (e.g. `-999`).
Point queries make this explicit in their return type/value:

| Method | No-data representation |
|---|---|
| `VoxelData.query_geometry` | returns `undefined` |
| `VoxelQuery.query_bundle` | that point's entry in the result array is `NaN` |
| `VoxelData.query_plane` / `query_arr16` / `query_arr_geometry` (PNG methods) | rendered as the bottom color of the variable's ramp (internally treated as `NaN` going into the color ramp, which clamps it) |

A variable with no declared no-data value (older/simpler SLPKs) never
produces `undefined`/`NaN` from this cause — every voxel is a real value.

## Picking the right `layer_id`

Don't assume the variable you want is id `0`. Inspect `layer.json`'s
`variables` array (`{ id, name, description, unit?, originalFormat, ... }`)
and match on `name` yourself — e.g. a layer might order its variables as
`[T, WindSpeed, Wind_x, Wind_y, Wind_z, p]`, making `WindSpeed` id `1`. Pass
that id consistently as `layerId` to `get_paths`, `make_layer`/`VoxelQuery`,
and every query call.

## Typical flows

### Flow A: full volume → PNGs (`get_paths` + `VoxelData`)

```js
const layerJson = await fetch(`${sceneServerUrl}/layers/0`).then(r => r.text());
const layerId = 1; // e.g. "WindSpeed" — found by inspecting layer.json's variables array

const urls = get_paths(sceneServerUrl, layerJson, layerId);
const values = await Promise.all(
  urls.map(u => fetch(u).then(r => r.arrayBuffer()).then(b => new Uint8Array(b)))
);

const voxelData = VoxelData.make_layer(layerJson, values, layerId);
const [xmin, xmax, ymin, ymax, zmin, zmax] = voxelData.get_corners();

// Only valid because layerId === 0 here; otherwise use query_geometry /
// query_arr_geometry instead (see the gotcha above).
const pngBytes = voxelData.query_plane(zIndex);
const blob = new Blob([pngBytes], { type: "image/png" });
```

### Flow B: memory-lean batch point queries (`VoxelQuery`)

```js
const q = new VoxelQuery(layerJson, layerId);
const plan = q.plan(xs, ys, zs); // Float32Arrays, world coordinates

const results = new Float32Array(xs.length).fill(NaN);
for (let i = 0; i < plan.bundle_ids.length; i++) {
  const [nx, ny, nz] = plan.bundle_ids[i].split("-").map(Number);
  const bytes = await fetch(
    `${sceneServerUrl}/variables/${layerId}/0/bundles/0/${plan.bundle_ids[i]}`
  ).then(r => r.arrayBuffer()).then(b => new Uint8Array(b));

  // Points assigned to this bundle:
  const idxs = [...plan.assignments].reduce((acc, bundleIdx, p) => {
    if (bundleIdx === i) acc.push(p);
    return acc;
  }, []);
  const bxs = Float32Array.from(idxs, p => xs[p]);
  const bys = Float32Array.from(idxs, p => ys[p]);
  const bzs = Float32Array.from(idxs, p => zs[p]);

  const values = q.query_bundle(nx, ny, nz, bytes, bxs, bys, bzs);
  idxs.forEach((p, k) => { results[p] = values[k]; }); // NaN entries = no-data
}
// points where plan.assignments[p] === 0xFFFFFFFF were outside the extent
// and were never assigned a bundle; results[p] stays NaN for those too.
```

## TypeScript

A `.d.ts` is generated alongside the JS output (`shearslice_wasm.d.ts`), so
all the signatures above are statically typed out of the box — no separate
`@types` package needed.

## Building

From the repo root:

```sh
make wasm
# = wasm-pack build shearslice_wasm --out-dir ../pkg/ --release --scope dplplugin
```

Requires `wasm-pack` and the `wasm32-unknown-unknown` Rust target
(`rustup target add wasm32-unknown-unknown`). Output lands in `pkg/` as an ES
module package (`@dplplugin/shearslice_wasm`), built by default for the
`bundler` target (Webpack/Vite/Rollup-style `import`); pass `-t web` or
`-t nodejs` to `wasm-pack build` directly if you need a different target's
init pattern.

## Known limitations

- Only `f32` voxel values are supported.
- No time-dimension support — always reads as if `t = 0`.
- `query_plane`/`query_arr16` only read variable id `0` regardless of the
  `layerId` the `VoxelData` was constructed with (see above).
- Variables/dimensions are selected by numeric id/index; no select-by-name
  helper is provided (see [Picking the right `layer_id`](#picking-the-right-layer_id)).
