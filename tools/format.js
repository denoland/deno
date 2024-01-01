#!/usr/bin/env -S deno run --unstable --allow-write --allow-read --allow-run --allow-net
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { join, ROOT_PATH } from "./util.js";

const subcommand = Deno.args.includes("--check") ? "check" : "fmt";
const configFile = join(ROOT_PATH, ".dprint.json");
const cmd = new Deno.Command("deno", {
  args: [
    "run",
    "-A",
    "--no-config",
    "npm:dprint@0.43.0",
    subcommand,
    "--config=" + configFile,
  ],
  cwd: ROOT_PATH,
  stdout: "piped",
  stderr: "inherit",
});

const { code, stdout } = await cmd.output();
// todo(dsherret): temporary until https://github.com/denoland/deno/pull/21359 gets released.
// Once it's released, just have stdout be inherited above and do `Deno.exit(code)` here.
const stdoutText = new TextDecoder().decode(stdout);
console.log(stdoutText);
if (stdoutText.length > 0) {
  Deno.exit(20);
} else {
  Deno.exit(code);
}
