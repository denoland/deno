#!/usr/bin/env -S deno run --allow-write --allow-read --allow-net --allow-env --config=tests/config/deno.json
// Copyright 2018-2025 the Deno authors. MIT license.
import { getPrebuilt } from "./util.js";

const args = Deno.args.slice();
for (const arg of args) {
  await getPrebuilt(arg);
}
