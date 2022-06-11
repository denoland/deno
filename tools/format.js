#!/usr/bin/env -S deno run --unstable --allow-write --allow-read --allow-run
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { getPrebuiltToolPath, join, ROOT_PATH } from "./util.js";

async function dprint() {
  const configFile = join(ROOT_PATH, ".dprint.json");
  const execPath = getPrebuiltToolPath("dprint");
  const { status } = await Deno.spawn(execPath, {
    args: ["fmt", "--config=" + configFile],
    stdout: "inherit",
    stderr: "inherit",
  });
  if (!status.success) {
    throw new Error("dprint failed");
  }
}

async function main() {
  await Deno.chdir(ROOT_PATH);
  await dprint();

  if (Deno.args.includes("--check")) {
    const { status, stdout } = await Deno.spawn("git", {
      args: ["status", "-uno", "--porcelain", "--ignore-submodules"],
      stderr: "inherit",
    });

    if (!status.success) {
      throw new Error("git status failed");
    }
    const out = new TextDecoder().decode(stdout);

    if (out) {
      console.log("run tools/format.js");
      console.log(out);
      Deno.exit(1);
    }
  }
}

await main();
