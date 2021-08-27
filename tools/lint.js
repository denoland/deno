#!/usr/bin/env -S deno run --unstable --allow-write --allow-read --allow-run
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  buildMode,
  getPrebuiltToolPath,
  getSources,
  join,
  ROOT_PATH,
} from "./util.js";

async function dlint() {
  const configFile = join(ROOT_PATH, ".dlint.json");
  const execPath = getPrebuiltToolPath("dlint");
  console.log("dlint");

  const sourceFiles = await getSources(ROOT_PATH, [
    "*.js",
    "*.ts",
    ":!:.github/mtime_cache/action.js",
    ":!:cli/tests/testdata/swc_syntax_error.ts",
    ":!:cli/tests/testdata/038_checkjs.js",
    ":!:cli/tests/testdata/error_008_checkjs.js",
    ":!:std/**/testdata/*",
    ":!:std/**/node_modules/*",
    ":!:cli/bench/node*.js",
    ":!:cli/compilers/wasm_wrap.js",
    ":!:cli/dts/**",
    ":!:cli/tests/testdata/encoding/**",
    ":!:cli/tests/testdata/error_syntax.js",
    ":!:cli/tests/unit/**",
    ":!:cli/tests/testdata/lint/**",
    ":!:cli/tests/testdata/tsc/**",
    ":!:cli/tsc/*typescript.js",
    ":!:test_util/wpt/**",
  ]);

  if (!sourceFiles.length) {
    return;
  }

  const chunks = splitToChunks(sourceFiles, `${execPath} run`.length);
  for (const chunk of chunks) {
    const p = Deno.run({
      cmd: [execPath, "run", "--config=" + configFile, ...chunk],
    });
    const { success } = await p.status();
    if (!success) {
      throw new Error("dlint failed");
    }
    p.close();
  }
}

// `prefer-primordials` has to apply only to files related to bootstrapping,
// which is different from other lint rules. This is why this dedicated function
// is needed.
async function dlintPreferPrimordials() {
  const execPath = getPrebuiltToolPath("dlint");
  console.log("prefer-primordials");

  const sourceFiles = await getSources(ROOT_PATH, [
    "runtime/**/*.js",
    "ext/**/*.js",
    "core/**/*.js",
    ":!:core/examples/**",
  ]);

  if (!sourceFiles.length) {
    return;
  }

  const chunks = splitToChunks(sourceFiles, `${execPath} run`.length);
  for (const chunk of chunks) {
    const p = Deno.run({
      cmd: [execPath, "run", "--rule", "prefer-primordials", ...chunk],
    });
    const { success } = await p.status();
    if (!success) {
      throw new Error("prefer-primordials failed");
    }
    p.close();
  }
}

function splitToChunks(paths, initCmdLen) {
  let cmdLen = initCmdLen;
  const MAX_COMMAND_LEN = 30000;
  const chunks = [[]];
  for (const p of paths) {
    if (cmdLen + p.length > MAX_COMMAND_LEN) {
      chunks.push([p]);
      cmdLen = initCmdLen;
    } else {
      chunks[chunks.length - 1].push(p);
      cmdLen += p.length;
    }
  }
  return chunks;
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
    await dlintPreferPrimordials();
    didLint = true;
  }

  if (Deno.args.includes("--rs")) {
    await clippy();
    didLint = true;
  }

  if (!didLint) {
    await dlint();
    await dlintPreferPrimordials();
    await clippy();
  }
}

await main();
