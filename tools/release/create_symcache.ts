// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import { createSymcache } from "jsr:@deno/panic@0.1.0";
import path from "node:path";

// Generate symcache for the current Deno executable.

let debugFile = Deno.execPath();

if (Deno.build.os === "windows") {
  debugFile = debugFile.replace(/\.exe$/, ".pdb");
} else if (Deno.build.os === "darwin") {
  const resolvedPath = Deno.realPathSync(`${debugFile}.dSYM`);
  const { name } = path.parse(resolvedPath);

  debugFile = path.join(resolvedPath, "Contents/Resources/DWARF", name);
}

const outfile = Deno.args[0];
if (!outfile) {
  console.error("Usage: ./target/release/deno -A create_symcache.ts <outfile>");
  Deno.exit(1);
}

const symcache = createSymcache(Deno.readFileSync(debugFile));
Deno.writeFileSync(outfile, symcache);
