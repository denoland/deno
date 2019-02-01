#!/usr/bin/env deno --allow-run --allow-write
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { exit, args } from "deno";
import { parse } from "./flags/mod.ts";
import { xrun, executableSuffix } from "./prettier/util.ts";

async function main(opts) {
  const args = [
    `deno${executableSuffix}`,
    "--allow-write",
    "--allow-run",
    "prettier/main.ts",
    "--ignore",
    "testdata",
    "--ignore",
    "vendor"
  ];

  if (opts.check) {
    args.push("--check");
  }

  exit((await xrun({ args }).status()).code);
}

main(parse(args));
