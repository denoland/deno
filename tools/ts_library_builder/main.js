#!/usr/bin/env node

// This modules bootstraps ts-node to allow on the fly transpiling of
// TypeScript while under NodeJS and performs an interpretation of
// process.argv to pass to the build_library main function.

require("ts-node").register({
  compilerOptions: {
    strict: true,
    target: "esnext"
  },
  // ts-node is finding the root `tsconfig.json` and not the one located this
  // directory, so to ensure re-producability, we will not attempt to load
  // a tsconfig.json, therefore skipProject = true
  skipProject: true
});

const path = require("path");

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

const buildRuntimeLib = require("./build_library").main;

buildRuntimeLib({
  basePath,
  buildPath,
  debug,
  outFile,
  silent
});
