// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import "./polyfill_globals.js";
import { createRequire } from "node:module";
import { toFileUrl } from "@std/path/to-file-url";
import process from "node:process";

// exclude argv[1], which is the path to this runner script.
// some tests expect that argv[1] is the path to the test file.
function patchArgv() {
  const out = [];
  for (let i = 0; i < process.argv.length; i++) {
    if (i === 1) {
      continue;
    }
    out.push(process.argv[i]);
  }
  process.argv = out;
}

const file = Deno.args[0];
if (!file) {
  throw new Error("No file provided");
}

patchArgv();

if (file.endsWith(".mjs")) {
  await import(toFileUrl(file).href);
} else {
  createRequire(import.meta.url)(file);
}
