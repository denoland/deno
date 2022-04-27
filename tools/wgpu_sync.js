#!/usr/bin/env -S deno run --unstable --allow-read --allow-write --allow-run
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { join, ROOT_PATH, walk } from "./util.js";

// const COMMIT = "c00e471274b6c21acda89b4b13d41742c0285d71"; // Release 12
const COMMIT = "c4aa3eaed020a640fec06b48f0a5ea93490d41bb"; // tip of PR (needs merge)
const REPO = "kvark/wgpu";
const V_WGPU = "0.12";
const TARGET_DIR = join(ROOT_PATH, "ext", "webgpu");

async function bash(subcmd, opts = {}) {
  const { status } = await Deno.spawn("bash", {
    ...opts,
    args: ["-c", subcmd],
    stdout: "inherit",
    sdterr: "inherit",
  });

  // Exit process on failure
  if (!status.success) {
    Deno.exit(status.code);
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

async function denoCoreVersion() {
  const coreCargo = join(ROOT_PATH, "core", "Cargo.toml");
  const contents = await Deno.readTextFile(coreCargo);
  return contents.match(/^version = "(\d+\.\d+\.\d+)"$/m)[1];
}

async function denoWebgpuVersion() {
  const coreCargo = join(ROOT_PATH, "runtime", "Cargo.toml");
  const contents = await Deno.readTextFile(coreCargo);
  return contents.match(
    /^deno_webgpu = { version = "(\d+\.\d+\.\d+)", path = "..\/ext\/webgpu" }$/m,
  )[1];
}

async function patchFile(path, patcher) {
  const data = await Deno.readTextFile(path);
  const patched = patcher(data);
  await Deno.writeTextFile(path, patched);
}

async function patchCargo() {
  const vDenoCore = await denoCoreVersion();
  const vDenoWebgpu = await denoWebgpuVersion();
  await patchFile(
    join(TARGET_DIR, "Cargo.toml"),
    (data) =>
      data
        .replace(/^version = .*/m, `version = "${vDenoWebgpu}"`)
        .replace(`edition = "2018"`, `edition = "2021"`)
        .replace(
          /^deno_core \= .*$/gm,
          `deno_core = { version = "${vDenoCore}", path = "../../core" }`,
        )
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

async function patchCopyrights() {
  const walker = walk(TARGET_DIR, { includeDirs: false });
  for await (const entry of walker) {
    await patchFile(
      entry.path,
      (data) =>
        data.replace(/^\/\/ Copyright 2018-2021/, "// Copyright 2018-2022"),
    );
  }
}

async function main() {
  await clearTargetDir();
  await checkoutUpstream();
  await patchCargo();
  await patchSrcLib();
  await patchCopyrights();
  await bash(join(ROOT_PATH, "tools", "format.js"));
}

await main();
