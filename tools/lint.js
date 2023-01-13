#!/usr/bin/env -S deno run --unstable --allow-write --allow-read --allow-run
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
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
    ":!:cli/tests/testdata/error_008_checkjs.js",
    ":!:cli/bench/http/node*.js",
    ":!:cli/bench/testdata/npm/*",
    ":!:cli/bench/testdata/express-router.js",
    ":!:cli/bench/testdata/react-dom.js",
    ":!:cli/compilers/wasm_wrap.js",
    ":!:cli/tsc/dts/**",
    ":!:cli/tests/testdata/encoding/**",
    ":!:cli/tests/testdata/error_syntax.js",
    ":!:cli/tests/testdata/fmt/**",
    ":!:cli/tests/testdata/npm/**",
    ":!:cli/tests/testdata/lint/**",
    ":!:cli/tests/testdata/run/**",
    ":!:cli/tests/testdata/tsc/**",
    ":!:cli/tsc/*typescript.js",
    ":!:cli/tsc/compiler.d.ts",
    ":!:test_util/wpt/**",
  ]);

  if (!sourceFiles.length) {
    return;
  }

  const chunks = splitToChunks(sourceFiles, `${execPath} run`.length);
  for (const chunk of chunks) {
    const cmd = new Deno.Command(execPath, {
      args: ["run", "--config=" + configFile, ...chunk],
      stdout: "inherit",
      stderr: "inherit",
    });
    const { code } = await cmd.output();

    if (code > 0) {
      throw new Error("dlint failed");
    }
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
    const cmd = new Deno.Command(execPath, {
      args: ["run", "--rule", "prefer-primordials", ...chunk],
      stdout: "inherit",
      stderr: "inherit",
    });
    const { code } = await cmd.output();

    if (code > 0) {
      throw new Error("prefer-primordials failed");
    }
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
  const cmd = ["clippy", "--all-targets", "--locked"];

  if (currentBuildMode != "debug") {
    cmd.push("--release");
  }

  const cargoCmd = new Deno.Command("cargo", {
    args: [
      ...cmd,
      "--",
      "-D",
      "warnings",
    ],
    stdout: "inherit",
    stderr: "inherit",
  });
  const { code } = await cargoCmd.output();

  if (code > 0) {
    throw new Error("clippy failed");
  }
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
