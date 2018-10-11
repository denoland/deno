import * as path from "path";
import { main as buildRuntimeLib } from "./build_library";

// this is very simplistic argument parsing, just enough to integrate into
// the build scripts, versus being very robust
let basePath = process.cwd();
let buildPath = path.join(basePath, "out", "debug");
let outFile = path.join(buildPath, "gen", "lib", "lib.d.ts");
let debug = false;
let silent = false;

process.argv.forEach((arg, i, argv) => {
  // tslint:disable-next-line:switch-default
  switch (arg) {
    case "--basePath":
      basePath = path.resolve(argv[i + 1]);
      break;
    case "--buildPath":
      buildPath = path.resolve(argv[i + 1]);
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
  outFile,
  silent
});
