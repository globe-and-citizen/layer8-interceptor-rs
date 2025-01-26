build-experimental:
	RUSTFLAGS='-C target-feature=+atomics,+bulk-memory' \
  		rustup run nightly \
		wasm-pack build --target bundler --release -- --features experimental -- -Z build-std=panic_abort,std && gawk -f post_build.gawk ./pkg/package.json > ./pkg/package.json.tmp && mv ./pkg/package.json.tmp ./pkg/package.json \
	 	gawk -f post_build.gawk ./pkg/layer8_interceptor_rs.js > ./pkg/layer8_interceptor_rs.js.tmp && mv ./pkg/layer8_interceptor_rs.js.tmp ./pkg/layer8_interceptor_rs.js

debug-experimental:
	RUSTFLAGS='-C target-feature=+atomics,+bulk-memory' \
  		rustup run nightly \
		wasm-pack build --target bundler --debug -- --features experimental -- -Z build-std=panic_abort,std && gawk -f post_build.gawk ./pkg/package.json > ./pkg/package.json.tmp && mv ./pkg/package.json.tmp ./pkg/package.json \
	 	gawk -f post_build.gawk ./pkg/layer8_interceptor_rs.js > ./pkg/layer8_interceptor_rs.js.tmp && mv ./pkg/layer8_interceptor_rs.js.tmp ./pkg/layer8_interceptor_rs.js

build:
	wasm-pack build --target bundler --release && gawk -f post_build.gawk ./pkg/package.json > ./pkg/package.json.tmp && mv ./pkg/package.json.tmp ./pkg/package.json \
 		gawk -f post_build.gawk ./pkg/layer8_interceptor_rs.js > ./pkg/layer8_interceptor_rs.js.tmp && mv ./pkg/layer8_interceptor_rs.js.tmp ./pkg/layer8_interceptor_rs.js

debug:
	wasm-pack build --target bundler --debug && gawk -f post_build.gawk ./pkg/package.json > ./pkg/package.json.tmp && mv ./pkg/package.json.tmp ./pkg/package.json \
	 	gawk -f post_build.gawk ./pkg/layer8_interceptor_rs.js > ./pkg/layer8_interceptor_rs.js.tmp && mv ./pkg/layer8_interceptor_rs.js.tmp ./pkg/layer8_interceptor_rs.js