#!/usr/bin/env bash
# TODO Port this to deno.
cargo build --release -vv
rm -rf gen gen.tar.gz
mkdir -p gen/bundle
cp target/release/gen/bundle/main.js gen/bundle/
cp target/release/gen/bundle/main.js.map gen/bundle/
cp target/release/gen/msg_generated.rs gen/
cp target/release/gen/snapshot_deno.bin gen/
cp target/release/obj/libdeno/libdeno.a gen/

tar cjvf gen.tar.bz2 gen/
#rm -rf target
#
#tar -cf gen.tar gen/
#bzip2 gen.tar
