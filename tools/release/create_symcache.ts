// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import { create_symcache } from "../../../panic/wasm/lib/rs_lib.js";

// Generate symcache for the current Deno executable.

let debugFile = Deno.execPath();

if (Deno.build.os === "windows") {
  debugFile = debugFile.replace(/\.exe$/, ".pdb");
}

const outfile = Deno.args[0];
if (!outfile) {
  console.error("Usage: ./target/release/deno -A create_symcache.ts <outfile>");
  Deno.exit(1);
}

const symcache = create_symcache(debugFile);
Deno.writeFileSync(outfile, symcache);
