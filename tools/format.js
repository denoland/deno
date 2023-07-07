#!/usr/bin/env -S deno run --unstable --allow-write --allow-read --allow-run
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { getPrebuiltToolPath, join, ROOT_PATH } from "./util.js";

const subcommand = Deno.args.includes("--check") ? "check" : "fmt";
const configFile = join(ROOT_PATH, ".dprint.json");
const execPath = getPrebuiltToolPath("dprint");
const cmd = new Deno.Command(execPath, {
  args: [subcommand, "--config=" + configFile],
  cwd: ROOT_PATH,
  stdout: "inherit",
  stderr: "inherit",
});

const { code } = await cmd.output();
Deno.exit(code);
