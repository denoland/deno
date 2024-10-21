#!/usr/bin/env -S deno run --allow-all --config=tests/config/deno.json
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { join, ROOT_PATH } from "./util.js";

const subcommand = Deno.args.includes("--check") ? "check" : "fmt";
const configFile = join(ROOT_PATH, ".dprint.json");
const cmd = new Deno.Command("deno", {
  args: [
    "run",
    "-A",
    "--no-config",
    "npm:dprint@0.47.2",
    subcommand,
    "--config=" + configFile,
  ],
  cwd: ROOT_PATH,
  stdout: "inherit",
  stderr: "inherit",
});

const { code } = await cmd.output();
Deno.exit(code);
