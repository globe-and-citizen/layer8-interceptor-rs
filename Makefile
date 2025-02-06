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