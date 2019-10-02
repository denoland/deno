#!/usr/bin/env -S deno run --allow-run --allow-write --allow-read --allow-env
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { exit, args, execPath } = Deno;
import { parse } from "./flags/mod.ts";
import { xrun } from "./prettier/util.ts";

async function main(opts): Promise<void> {
  const args = [
    execPath(),
    "run",
    "--allow-write",
    "--allow-read",
    "prettier/main.ts",
    "--ignore",
    "node_modules",
    "--ignore",
    "**/testdata",
    "--ignore",
    "**/vendor",
    "--write"
  ];

  if (opts.check) {
    args.push("--check");
  }

  args.push(".");

  exit((await xrun({ args }).status()).code);
}

main(parse(args));
