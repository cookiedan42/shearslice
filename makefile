###############################################################################
wasm:
	wasm-pack build shearslice_wasm --out-dir ../pkg/ --release --scope dpl

wasm-nc:
	wasm-pack build ncvoxel_wasm --out-dir ../pkg-nc/ --release --scope dplplugin

###############################################################################
# Browser tests. wasm-pack's own driver launch fails on some machines, so start
# chromedriver ourselves and point the runner at it (CHROMEDRIVER_REMOTE).
# Uses `chromedriver` from PATH, else the most recently downloaded one in wasm-pack's cache
# (its major version must match your installed Chrome).
# Override with: make wasm-test CHROMEDRIVER=/path/to/chromedriver PORT=9515
PORT ?= 9515
CHROMEDRIVER ?= $(or $(shell command -v chromedriver),$(shell ls -t $(HOME)/Library/Caches/.wasm-pack/chromedriver-*/chromedriver $(HOME)/.cache/.wasm-pack/chromedriver-*/chromedriver 2>/dev/null | head -1))

.PHONY: wasm-test
wasm-test:
	@test -n "$(CHROMEDRIVER)" -a -x "$(CHROMEDRIVER)" || { echo "chromedriver not found; install it or pass CHROMEDRIVER=/path"; exit 1; }
	@"$(CHROMEDRIVER)" --port=$(PORT) >/dev/null 2>&1 & pid=$$!; \
	trap 'kill $$pid 2>/dev/null' EXIT INT TERM; \
	sleep 1; \
	env -u HTTP_PROXY -u HTTPS_PROXY -u ALL_PROXY -u http_proxy -u https_proxy -u all_proxy \
	  CHROMEDRIVER_REMOTE=http://localhost:$(PORT) \
	  wasm-pack test --headless --chrome shearslice_wasm
	wasm-pack build shearslice_wasm --out-dir ../pkg/ --release --scope dplplugin 
###############################################################################
