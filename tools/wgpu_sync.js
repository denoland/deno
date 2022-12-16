#!/usr/bin/env -S deno run --unstable --allow-read --allow-write --allow-run
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { join, ROOT_PATH } from "./util.js";

const COMMIT = "076df1a56812eee01614b7a3a4c88798012e79ab";
const REPO = "gfx-rs/wgpu";
const V_WGPU = "0.13";
const TARGET_DIR = join(ROOT_PATH, "ext", "webgpu");

async function bash(subcmd, opts = {}) {
  const { success, code } = await new Deno.Command("bash", {
    ...opts,
    args: ["-c", subcmd],
    stdout: "inherit",
    sdterr: "inherit",
  }).output();

  // Exit process on failure
  if (!success) {
    Deno.exit(code);
  }
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

async function denoWebgpuVersion() {
  const coreCargo = join(ROOT_PATH, "Cargo.toml");
  const contents = await Deno.readTextFile(coreCargo);
  return contents.match(
    /^deno_webgpu = { version = "(\d+\.\d+\.\d+)", path = ".\/ext\/webgpu" }$/m,
  )[1];
}

async function patchFile(path, patcher) {
  const data = await Deno.readTextFile(path);
  const patched = patcher(data);
  await Deno.writeTextFile(path, patched);
}

async function patchCargo() {
  const vDenoWebgpu = await denoWebgpuVersion();
  await patchFile(
    join(TARGET_DIR, "Cargo.toml"),
    (data) =>
      data
        .replace(/^version = .*/m, `version = "${vDenoWebgpu}"`)
        .replace(
          /^wgpu-core \= .*$/gm,
          `wgpu-core = { version = "${V_WGPU}", features = ["trace", "replay", "serde"] }`,
        )
        .replace(
          /^wgpu-types \= .*$/gm,
          `wgpu-types = { version = "${V_WGPU}", features = ["trace", "replay", "serde"] }`,
        ),
    // .replace(
    //   /^wgpu-core \= .*$/gm,
    //   `wgpu-core = { git = "https://github.com/${REPO}", rev = "${COMMIT}", features = ["trace", "replay", "serde"] }`,
    // )
    // .replace(
    //   /^wgpu-types \= .*$/gm,
    //   `wgpu-types = { git = "https://github.com/${REPO}", rev = "${COMMIT}", features = ["trace", "replay", "serde"] }`,
    // )
  );
}

async function patchSrcLib() {
  await patchFile(
    join(TARGET_DIR, "src", "lib.rs"),
    (data) =>
      data.replace(`prefix "deno:deno_webgpu",`, `prefix "deno:ext/webgpu",`),
  );
}

async function main() {
  await clearTargetDir();
  await checkoutUpstream();
  await patchCargo();
  await patchSrcLib();
  await bash(join(ROOT_PATH, "tools", "format.js"));
}

await main();
