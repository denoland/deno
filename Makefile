TS_FILES = \
	dispatch.ts \
	globals.ts \
	main.ts \
	msg.pb.d.ts \
	msg.pb.js \
	os.ts \
	runtime.ts \
	timers.ts \
	tsconfig.json \
	types.ts \
	url.js \
	util.ts \
	v8_source_maps.ts \
	v8worker2.d.ts

GO_FILES = \
	assets.go \
	deno_dir.go \
	deno_dir_test.go \
	echo.go \
	dispatch.go \
	main.go \
	msg.pb.go \
	os.go \
	os_test.go \
	timers.go \
	util.go

deno: msg.pb.go $(GO_FILES)
	go build -o deno

assets.go: dist/main.js
	cp node_modules/typescript/lib/lib.d.ts dist/
	cp deno.d.ts dist/
	go-bindata -pkg main -o assets.go dist/

msg.pb.go: msg.proto
	protoc --go_out=. msg.proto

msg.pb.js: msg.proto node_modules
	./node_modules/.bin/pbjs -t static-module -w commonjs -o msg.pb.js msg.proto

msg.pb.d.ts: msg.pb.js node_modules
	./node_modules/.bin/pbts -o msg.pb.d.ts msg.pb.js

dist/main.js: $(TS_FILES) node_modules
	./node_modules/.bin/tsc --noEmit # Only for type checking.
	./node_modules/.bin/parcel build --out-dir=dist/ --no-minify main.ts

node_modules:
	yarn

clean:
	-rm -f deno assets.go msg.pb.go msg.pb.js msg.pb.d.ts
	-rm -rf dist/

distclean: clean
	-rm -rf node_modules/

lint: node_modules
	yarn lint
	go vet

fmt: node_modules
	yarn fmt
	go fmt
	clang-format msg.proto -i

test: deno
	go test -v

.PHONY: test lint clean distclean
