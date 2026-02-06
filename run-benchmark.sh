#!/bin/bash
nix-shell -p oha bun --run "deno run -A compare-runtimes.ts --trials 1 --candidate deno-libuv=deno@./target/release/deno --candidate deno-stable=deno@deno --candidate node=node@node --candidate bun=bun@bun -- --bytes 1g --oha-duration 5s"
