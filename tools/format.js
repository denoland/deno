#!/usr/bin/env -S deno run --unstable --allow-write --allow-read --allow-run
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { getPrebuiltToolPath, getSources, join, ROOT_PATH } from "./util.js";

async function dprint() {
  const configFile = join(ROOT_PATH, ".dprintrc.json");
  const execPath = getPrebuiltToolPath("dprint");
  console.log("dprint");
  const p = Deno.run({
    cmd: [execPath, "fmt", "--config=" + configFile],
  });
  const { success } = await p.status();
  if (!success) {
    throw new Error("dprint failed");
  }
  p.close();
}

async function rustfmt() {
  const configFile = join(ROOT_PATH, ".rustfmt.toml");
  const sourceFiles = await getSources(ROOT_PATH, ["*.rs"]);

  if (!sourceFiles.length) {
    return;
  }

  console.log(`rustfmt ${sourceFiles.length} file(s)`);
  const p = Deno.run({
    cmd: ["rustfmt", "--config-path=" + configFile, "--", ...sourceFiles],
  });
  const { success } = await p.status();
  if (!success) {
    throw new Error("rustfmt failed");
  }
  p.close();
}

async function main() {
  await Deno.chdir(ROOT_PATH);
  await dprint();
  await rustfmt();

  if (Deno.args.includes("--check")) {
    const git = Deno.run({
      cmd: ["git", "status", "-uno", "--porcelain", "--ignore-submodules"],
      stdout: "piped",
    });

    const { success } = await git.status();
    if (!success) {
      throw new Error("git status failed");
    }
    const out = new TextDecoder().decode(await git.output());
    git.close();

    if (out) {
      console.log("run tools/format.js");
      console.log(out);
      Deno.exit(1);
    }
  }
}

await main();
