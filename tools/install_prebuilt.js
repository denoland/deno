#!/usr/bin/env -S deno run --unstable --allow-write --allow-read --allow-net --config=tests/config/deno.json
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { getPrebuilt } from "./util.js";

const args = Deno.args.slice();
for (const arg of args) {
  await getPrebuilt(arg);
}
