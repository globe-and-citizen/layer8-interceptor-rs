build-experimental:
	wasm-pack build --target bundler --release -- --features experimental && gawk -f post_build.gawk ./pkg/package.json > ./pkg/package.json.tmp && mv ./pkg/package.json.tmp ./pkg/package.json \
	 	gawk -f post_build.gawk ./pkg/layer8_interceptor_rs.js > ./pkg/layer8_interceptor_rs.js.tmp && mv ./pkg/layer8_interceptor_rs.js.tmp ./pkg/layer8_interceptor_rs.js

debug-experimental:
	wasm-pack build --target bundler --debug -- --features experimental && gawk -f post_build.gawk ./pkg/package.json > ./pkg/package.json.tmp && mv ./pkg/package.json.tmp ./pkg/package.json \
	 	gawk -f post_build.gawk ./pkg/layer8_interceptor_rs.js > ./pkg/layer8_interceptor_rs.js.tmp && mv ./pkg/layer8_interceptor_rs.js.tmp ./pkg/layer8_interceptor_rs.js

build:
	wasm-pack build --target bundler --release && gawk -f post_build.gawk ./pkg/package.json > ./pkg/package.json.tmp && mv ./pkg/package.json.tmp ./pkg/package.json \
 		gawk -f post_build.gawk ./pkg/layer8_interceptor_rs.js > ./pkg/layer8_interceptor_rs.js.tmp && mv ./pkg/layer8_interceptor_rs.js.tmp ./pkg/layer8_interceptor_rs.js

debug:
	wasm-pack build --target bundler --debug && gawk -f post_build.gawk ./pkg/package.json > ./pkg/package.json.tmp && mv ./pkg/package.json.tmp ./pkg/package.json \
	 	gawk -f post_build.gawk ./pkg/layer8_interceptor_rs.js > ./pkg/layer8_interceptor_rs.js.tmp && mv ./pkg/layer8_interceptor_rs.js.tmp ./pkg/layer8_interceptor_rs.js

# FIXME: it would be nice to have a `cargo test ...` with no reliance on nodejs
# We assume in running tests that if `MOCK_SERVER_PORT` is set, the mock server is running
# This recipe if hacky and will fail if address 8000 or 9999 is not available
test: check-nodejs
	(cd ./tests && npm install && npm run dev) & (sleep 5 && MOCK_SERVER_PORT=9999 WASM_BINDGEN_USE_BROWSER=1 wasm-pack test --chrome)

check-nodejs:
	which npm | grep -q node && echo "Node.js is installed" || (echo "Node.js is not installed" && exit 1)
