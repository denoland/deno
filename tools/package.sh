#!/usr/bin/env bash
# TODO Port this to deno.
cargo build --release -vv
mkdir -p gen/bundle
cp target/release/gen/bundle/main.js gen/bundle/
cp target/release/gen/bundle/main.js.map gen/bundle/
cp target/release/gen/msg_generated.rs gen/
cp target/release/gen/snapshot_deno.bin gen/
cp target/release/obj/libdeno/libdeno.a gen/
CARGO_PACKAGE=1 cargo package --allow-dirty -vv
