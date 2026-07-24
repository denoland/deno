#!/usr/bin/env -S deno run -A
// Copyright 2018-2026 the Deno authors. MIT license.

// This script is used to generate the @types/deno package on DefinitelyTyped.

import $ from "jsr:@david/dax@0.42.0";
import * as semver from "jsr:@std/semver@1.0.3";
import { generateDenoTypesDts } from "./release/types_dts.ts";

const rootDir = $.path(import.meta.dirname!).parentOrThrow();
const definitelyTypedDir = rootDir.join(
  "../DefinitelyTyped/types/deno/",
);

if (!definitelyTypedDir.existsSync()) {
  throw new Error(`Makes sure ${definitelyTypedDir} exists.`);
}

const denoExec = rootDir.join(
  "target/debug/deno" + (Deno.build.os === "windows" ? ".exe" : ""),
);

$.logStep("Building Deno executable...");
await $`cargo build`;

$.logStep("Creating declaration file...");
await createDenoDtsFile();
$.logStep("Updating package.json...");
await updatePkgJson();
$.logStep("Formatting...");
await $`pnpm dprint fmt`.cwd(definitelyTypedDir);

async function createDenoDtsFile() {
  const text = await $`${denoExec} types`.text();
  const dts = generateDenoTypesDts(text);
  definitelyTypedDir.join("index.d.ts").writeTextSync(dts);
}

async function updatePkgJson() {
  const pkgJsonFile = definitelyTypedDir.join("package.json");
  const obj = pkgJsonFile.readJsonSync();
  const version = semver.parse(await getDenoVersion());
  version.patch = 9999;
  version.prerelease = undefined;
  version.build = undefined;
  // deno-lint-ignore no-explicit-any
  (obj as any).version = semver.format(version);
  pkgJsonFile.writeTextSync(JSON.stringify(obj, undefined, 4) + "\n"); // 4 spaces indent
}

async function getDenoVersion() {
  const text = await $`${denoExec} -v`.text();
  return text.match(/deno (.*)/)![1];
}
