#!/usr/bin/env -S deno run -A --lock=tools/deno.lock.json
// Copyright 2018-2026 the Deno authors. MIT license.

import { DenoWorkspace } from "./deno_workspace.ts";
import { $ } from "./deps.ts";

const crateName = Deno.args[0];
const bumpKind = Deno.args[1];

if (!crateName || !bumpKind) {
  $.logError(
    "Usage: bump_single_crate_version.ts <crate_name> <patch|minor|major>",
  );
  Deno.exit(1);
}

if (!["patch", "minor", "major"].includes(bumpKind)) {
  $.logError(
    `Invalid bump kind: ${bumpKind}. Must be one of: patch, minor, major`,
  );
  Deno.exit(1);
}

const workspace = await DenoWorkspace.load();

let crate;
try {
  crate = workspace.getCrate(crateName);
} catch {
  $.logError(`Crate '${crateName}' not found in workspace.`);
  $.logError("Available crates:");
  for (const c of workspace.crates) {
    $.logError(`  - ${c.name} (${c.version})`);
  }
  Deno.exit(1);
}

const oldVersion = crate.version;
$.logStep(`Bumping ${crateName} from ${oldVersion} (${bumpKind})...`);

await crate.increment(bumpKind as "patch" | "minor" | "major");

$.logStep(`Bumped ${crateName}: ${oldVersion} -> ${crate.version}`);

// Update the lock file
$.logStep("Updating Cargo.lock...");
await workspace.getCliCrate().cargoUpdate("--workspace");

$.logStep("Done!");
