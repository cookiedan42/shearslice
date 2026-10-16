1. different data types, currently only f32 values are supported  
1. time variable is not supported yet, assumes that we are using t=0 for a variable with the time values  
1. select variables by name, currently only selecting by index is supported  
1. select coordinates by name, currently only selecting by index is supported




1. when out of extent, return None (NaN for wasm)