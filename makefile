###############################################################################
wasm:
	wasm-pack build shearslice_wasm --out-dir ../pkg/ --release --scope dplplugin 

wasm-nc:
	wasm-pack build ncvoxel_wasm --out-dir ../pkg-nc/ --release --scope dplplugin
###############################################################################
