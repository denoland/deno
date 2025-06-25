#!/usr/bin/env -S deno run --allow-all --config=tests/config/deno.json
// Copyright 2018-2025 the Deno authors. MIT license.
import { join, resolve } from "./util.js";

const currentDir = import.meta.dirname;
const testsDir = resolve(currentDir, "../tests/");
const args = [
  "-A",
  "--config",
  join(testsDir, "config/deno.json"),
  join(testsDir, "node_compat/run_all_test_unmodified.ts"),
];

let filterIdx = Deno.args.indexOf("--filter");
if (filterIdx === -1) {
  filterIdx = Deno.args.indexOf("-f");
}
if (filterIdx !== -1) {
  args.push("--filter");
  args.push(Deno.args.at(filterIdx + 1));
}

await new Deno.Command(Deno.execPath(), {
  args,
  stdout: "inherit",
  stderr: "inherit",
}).spawn();
