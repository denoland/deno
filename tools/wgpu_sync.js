#!/usr/bin/env -S deno run --unstable --allow-read --allow-write --allow-run --config=tests/config/deno.json
// Copyright 2018-2026 the Deno authors. MIT license.

import { join, ROOT_PATH } from "./util.js";

const COMMIT = "ae87ffe28041a7ebd82d8d3c2fa0e2343f0f0234";
const REPO = "gfx-rs/wgpu";
const V_WGPU = "29.0.1";
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

async function patchReadme() {
  const sourceSection = `## Source

The canonical source is considered to be https://github.com/gfx-rs/wgpu@trunk/deno_webgpu,
even though they are maintained separately but synced occasionally using the script
https://github.com/denoland/deno/blob/main/tools/wgpu_sync.js
`;

  await patchFile(
    join(TARGET_DIR, "README.md"),
    (data) => {
      // Drop any existing Source section so re-runs stay idempotent.
      const stripped = data.replace(
        /\n## Source\n[\s\S]*?(?=\n## |\n*$)/,
        "",
      );
      // Insert before the first subsequent "## " heading, or append.
      const headingMatch = stripped.match(/\n## /);
      if (headingMatch) {
        const idx = headingMatch.index + 1;
        return stripped.slice(0, idx) + sourceSection + "\n" +
          stripped.slice(idx);
      }
      return stripped.replace(/\n*$/, "\n\n") + sourceSection;
    },
  );
}

async function patchCargo() {
  const vDenoWebgpu = await denoWebgpuVersion();
  await patchFile(
    join(TARGET_DIR, "Cargo.toml"),
    (data) =>
      data
        .replace(/^version = .*/m, `version = "${vDenoWebgpu}"`)
        .replace(/^authors = .*/m, `authors.workspace = true`)
        .replace(/^license = .*/m, `license.workspace = true`)
        .replace(/^repository = .*/m, `repository.workspace = true`)
        .replace(
          /^serde = { workspace = true, features = ["derive"] }/m,
          `serde.workspace = true`,
        )
        .replace(
          /^tokio = { workspace = true, features = ["full"] }/m,
          `tokio.workspace = true`,
        ),
  );

  await patchFile(
    join(ROOT_PATH, "Cargo.toml"),
    (data) =>
      data
        .replace(/^wgpu-core = .*/m, `wgpu-core = "${V_WGPU}"`)
        .replace(/^wgpu-types = .*/m, `wgpu-types = "${V_WGPU}"`),
  );
}

async function main() {
  await clearTargetDir();
  await checkoutUpstream();
  await patchCargo();
  await patchReadme();
  await bash(join(ROOT_PATH, "tools", "format.js"));
}

await main();
