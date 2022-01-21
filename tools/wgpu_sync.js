#!/usr/bin/env -S deno run --unstable --allow-read --allow-write --allow-run
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { join, ROOT_PATH } from "./util.js";

// const COMMIT = "c00e471274b6c21acda89b4b13d41742c0285d71"; // Release 12
const COMMIT = "c4aa3eaed020a640fec06b48f0a5ea93490d41bb"; // tip of PR (needs merge)
const REPO = "kvark/wgpu";
// const V_WGPU = "0.12.0";
const TARGET_DIR = join(ROOT_PATH, "ext", "webgpu");

async function bash(subcmd, opts = {}) {
  const p = Deno.run({ ...opts, cmd: ["bash", "-c", subcmd] });

  // Exit process on failure
  const { success, code } = await p.status();
  if (!success) {
    Deno.exit(code);
  }
  // Cleanup
  p.close();
}

async function clearTargetDir() {
  await bash(`rm -r ${TARGET_DIR}/*`);
}

async function checkoutUpstream() {
  // Path of deno_webgpu inside the TAR
  const tarPrefix = `${REPO.replace("/", "-")}-${
    COMMIT.slice(0, 7)
  }/deno_webgpu/`;
  const cmd =
    `curl -L https://api.github.com/repos/${REPO}/tarball/${COMMIT} | tar -C '${TARGET_DIR}' -xzvf - --strip=2 '${tarPrefix}'`;
  // console.log(cmd);
  await bash(cmd);
}

async function denoCoreVersion() {
  const coreCargo = join(ROOT_PATH, "core", "Cargo.toml");
  const contents = await Deno.readTextFile(coreCargo);
  return contents.match(/^version = "(\d+\.\d+\.\d+)"$/m)[1];
}

async function patchCargo() {
  const vDenoCore = await denoCoreVersion();

  const webgpuCargo = join(ROOT_PATH, "ext", "webgpu", "Cargo.toml");
  const data = await Deno.readTextFile(webgpuCargo);

  // Patch ext/webgpu/Cargo.toml's contents
  const patched = data
    .replace(`version = "0.17.0"`, `version = "0.33.0"`)
    .replace(`edition = "2018"`, `edition = "2021"`)
    .replace(
      /^deno_core \= .*$/gm,
      `deno_core = { version = "${vDenoCore}", path = "../../core" }`,
    )
    // .replace(/^wgpu-core \= .*$/gm, `wgpu-core = { version = "${V_WGPU}", features = ["trace", "replay", "serde"] }`)
    // .replace(/^wgpu-types \= .*$/gm, `wgpu-types = { version = "${V_WGPU}", features = ["trace", "replay", "serde"] }`)
    .replace(
      /^wgpu-core \= .*$/gm,
      `wgpu-core = { git = "https://github.com/${REPO}", rev = "${COMMIT}", features = ["trace", "replay", "serde"] }`,
    )
    .replace(
      /^wgpu-types \= .*$/gm,
      `wgpu-types = { git = "https://github.com/${REPO}", rev = "${COMMIT}", features = ["trace", "replay", "serde"] }`,
    );

  await Deno.writeTextFile(webgpuCargo, patched);
}

async function patchSrcLib() {
  const srcLib = join(ROOT_PATH, "ext", "webgpu", "src", "lib.rs");
  const data = await Deno.readTextFile(srcLib);

  // Patch ext/webgpu/src/lib.rs's contents
  const patched = data
    .replace(`prefix "deno:deno_webgpu",`, `prefix "deno:ext/webgpu",`);

  await Deno.writeTextFile(srcLib, patched);
}

async function main() {
  await clearTargetDir();
  await checkoutUpstream();
  await patchCargo();
  await patchSrcLib();
  await bash(join(ROOT_PATH, "tools", "format.js"));
}

await main();
