#!/usr/bin/env -S deno run --unstable --allow-write --allow-read --allow-run
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  buildMode,
  getPrebuiltToolPath,
  getSources,
  ROOT_PATH,
} from "./util.js";

async function dlint() {
  const execPath = getPrebuiltToolPath("dlint");
  console.log("dlint");

  const sourceFiles = await getSources(ROOT_PATH, [
    "*.js",
    "*.ts",
    ":!:cli/tests/swc_syntax_error.ts",
    ":!:cli/tests/038_checkjs.js",
    ":!:cli/tests/error_008_checkjs.js",
    ":!:std/**/testdata/*",
    ":!:std/**/node_modules/*",
    ":!:cli/bench/node*.js",
    ":!:cli/compilers/wasm_wrap.js",
    ":!:cli/dts/**",
    ":!:cli/tests/encoding/**",
    ":!:cli/tests/error_syntax.js",
    ":!:cli/tests/lint/**",
    ":!:cli/tests/tsc/**",
    ":!:cli/tsc/*typescript.js",
    ":!:cli/tests/wpt/**",
  ]);

  if (!sourceFiles.length) {
    return;
  }

  const MAX_COMMAND_LEN = 30000;
  const preCommand = [execPath, "run"];
  const chunks = [[]];
  let cmdLen = preCommand.join(" ").length;
  for (const f of sourceFiles) {
    if (cmdLen + f.length > MAX_COMMAND_LEN) {
      chunks.push([f]);
      cmdLen = preCommand.join(" ").length;
    } else {
      chunks[chunks.length - 1].push(f);
      cmdLen = preCommand.join(" ").length;
    }
  }
  for (const chunk of chunks) {
    const p = Deno.run({
      cmd: [execPath, "run", ...chunk],
    });
    const { success } = await p.status();
    if (!success) {
      throw new Error("dlint failed");
    }
    p.close();
  }
}

async function clippy() {
  console.log("clippy");

  const currentBuildMode = buildMode();
  const cmd = ["cargo", "clippy", "--all-targets", "--locked"];

  if (currentBuildMode != "debug") {
    cmd.push("--release");
  }

  const p = Deno.run({
    cmd: [...cmd, "--", "-D", "clippy::all"],
  });
  const { success } = await p.status();
  if (!success) {
    throw new Error("clippy failed");
  }
  p.close();
}

async function main() {
  await Deno.chdir(ROOT_PATH);

  let didLint = false;

  if (Deno.args.includes("--js")) {
    await dlint();
    didLint = true;
  }

  if (Deno.args.includes("--rs")) {
    await clippy();
    didLint = true;
  }

  if (!didLint) {
    await dlint();
    await clippy();
  }
}

await main();
