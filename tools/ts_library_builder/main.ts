// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as path from "path";
import { main as buildRuntimeLib } from "./build_library";

// this is very simplistic argument parsing, just enough to integrate into
// the build scripts, versus being very robust
let basePath = process.cwd();
let buildPath = path.join(basePath, "target", "debug");
let outFile = path.join(buildPath, "gen", "lib", "lib.d.ts");
let inline: string[] = [];
let debug = false;
let silent = false;

process.argv.forEach((arg, i, argv) => {
  switch (arg) {
    case "--basePath":
      basePath = path.resolve(argv[i + 1]);
      break;
    case "--buildPath":
      buildPath = path.resolve(argv[i + 1]);
      break;
    case "--inline":
      inline = argv[i + 1].split(",").map(filename => {
        return path.resolve(filename);
      });
      break;
    case "--outFile":
      outFile = path.resolve(argv[i + 1]);
      break;
    case "--debug":
      debug = true;
      break;
    case "--silent":
      silent = true;
      break;
  }
});

buildRuntimeLib({
  basePath,
  buildPath,
  debug,
  inline,
  inputs: [
    "node_modules/typescript/lib/lib.esnext.d.ts",
    "js/deno.ts",
    "js/globals.ts"
  ],
  declareAsLet: ["onmessage"],
  outFile,
  silent
});
