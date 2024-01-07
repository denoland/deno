#!/usr/bin/env -S deno run --unstable --allow-write --allow-read --allow-run --allow-net
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { buildMode, getPrebuilt, getSources, join, ROOT_PATH } from "./util.js";
import { checkCopyright } from "./copyright_checker.js";

const promises = [];

let js = Deno.args.includes("--js");
let rs = Deno.args.includes("--rs");
if (!js && !rs) {
  js = true;
  rs = true;
}

if (js) {
  promises.push(dlint());
  promises.push(dlintPreferPrimordials());
}

if (rs) {
  promises.push(clippy());
}

if (js && rs) {
  promises.push(checkCopyright());
}

const results = await Promise.allSettled(promises);
for (const result of results) {
  if (result.status === "rejected") {
    console.error(result.reason);
    Deno.exit(1);
  }
}

async function dlint() {
  const configFile = join(ROOT_PATH, ".dlint.json");
  const execPath = await getPrebuilt("dlint");

  const sourceFiles = await getSources(ROOT_PATH, [
    "*.js",
    "*.ts",
    ":!:.github/mtime_cache/action.js",
    ":!:cli/tests/testdata/swc_syntax_error.ts",
    ":!:cli/tests/testdata/error_008_checkjs.js",
    ":!:cli/bench/testdata/npm/*",
    ":!:cli/bench/testdata/express-router.js",
    ":!:cli/bench/testdata/react-dom.js",
    ":!:cli/compilers/wasm_wrap.js",
    ":!:cli/tsc/dts/**",
    ":!:cli/tests/testdata/encoding/**",
    ":!:cli/tests/testdata/error_syntax.js",
    ":!:cli/tests/testdata/file_extensions/ts_with_js_extension.js",
    ":!:cli/tests/testdata/fmt/**",
    ":!:cli/tests/testdata/npm/**",
    ":!:cli/tests/testdata/lint/**",
    ":!:cli/tests/testdata/run/**",
    ":!:cli/tests/testdata/tsc/**",
    ":!:cli/tests/testdata/test/glob/**",
    ":!:cli/tsc/*typescript.js",
    ":!:cli/tsc/compiler.d.ts",
    ":!:test_util/wpt/**",
  ]);

  if (!sourceFiles.length) {
    return;
  }

  const chunks = splitToChunks(sourceFiles, `${execPath} run`.length);
  const pending = [];
  for (const chunk of chunks) {
    const cmd = new Deno.Command(execPath, {
      cwd: ROOT_PATH,
      args: ["run", "--config=" + configFile, ...chunk],
      // capture to not conflict with clippy output
      stderr: "piped",
    });
    pending.push(
      cmd.output().then(({ stderr, code }) => {
        if (code > 0) {
          const decoder = new TextDecoder();
          console.log("\n------ dlint ------");
          console.log(decoder.decode(stderr));
          throw new Error("dlint failed");
        }
      }),
    );
  }
  await Promise.all(pending);
}

// `prefer-primordials` has to apply only to files related to bootstrapping,
// which is different from other lint rules. This is why this dedicated function
// is needed.
async function dlintPreferPrimordials() {
  const execPath = await getPrebuilt("dlint");
  const sourceFiles = await getSources(ROOT_PATH, [
    "runtime/**/*.js",
    "ext/**/*.js",
    "ext/node/polyfills/*.mjs",
    "ext/node/polyfills/*.ts",
    ":!:ext/node/polyfills/*.d.ts",
    "core/*.js",
    ":!:core/*_test.js",
    ":!:core/examples/**",
  ]);

  if (!sourceFiles.length) {
    return;
  }

  const chunks = splitToChunks(sourceFiles, `${execPath} run`.length);
  for (const chunk of chunks) {
    const cmd = new Deno.Command(execPath, {
      cwd: ROOT_PATH,
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
  const currentBuildMode = buildMode();
  const cmd = ["clippy", "--all-targets", "--all-features", "--locked"];

  if (currentBuildMode != "debug") {
    cmd.push("--release");
  }

  const cargoCmd = new Deno.Command("cargo", {
    cwd: ROOT_PATH,
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
