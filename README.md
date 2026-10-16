# shearslice  

A library for slicing a 3D data along various elevations.

- Rust core + query engine: `shearslice_core/`
- WASM/JavaScript bindings: `shearslice_wasm/` — see
  **[shearslice_wasm/README.md](shearslice_wasm/README.md)** for the
  JS/TypeScript-facing API docs (installing, `VoxelData`/`VoxelQuery`,
  no-data handling, usage examples).

## Functionality

### data assembly  
- [x] querying slpk based on index
- [x] querying slpk based on world coordinates
### data visualization
- [x] assemble a png image from the data
- [x] write a gif from slices of the data
- [x] use color ramp from slpk file
### Frontend integation
- [x] obtaining required buffers in webapp
- [x] displaying georeferenced png file in webapp
- [x] web assembly compilation

### Performance
- [ ] lazy load of data
- [ ] parallel construction of slice parts  
