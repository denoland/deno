#!/usr/bin/env -S deno run --allow-write --allow-read --allow-run --allow-net
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { join, ROOT_PATH } from "./util.js";

const subcommand = Deno.args.includes("--check") ? "check" : "fmt";
const configFile = join(ROOT_PATH, ".dprint.json");
const cmd = new Deno.Command("deno", {
  args: [
    "run",
    "-A",
    "--no-config",
    "npm:dprint@0.45.1",
    subcommand,
    "--config=" + configFile,
  ],
  cwd: ROOT_PATH,
  stdout: "inherit",
  stderr: "inherit",
});

const { code } = await cmd.output();
Deno.exit(code);
