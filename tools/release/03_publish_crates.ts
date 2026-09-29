#!/usr/bin/env -S deno run -A --lock=tools/deno.lock.json
// Copyright 2018-2026 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import { DenoWorkspace } from "./deno_workspace.ts";
import { $, getCratesPublishOrder } from "./deps.ts";

$.logStep(`Running cargo publish...`);

const workspace = await DenoWorkspace.load();
const cliCrate = workspace.getCliCrate();

const dependencyCrates = getCratesPublishOrder(
  workspace.getCliDependencyCrates(),
);

try {
  for (const [i, crate] of dependencyCrates.entries()) {
    // `--no-verify` because the dependency crates can't be built on their own:
    // they depend on `deno_core` with default features off, so nothing selects
    // an engine for the `deno_v8` facade and it hits its `compile_error!`.
    // Engine selection happens at the top of the tree (see `deno`'s `v8` and
    // `quickjs` features), which cargo's standalone tarball verification can't
    // reproduce. The `deno` crate itself is still verified below.
    await crate.publish("--no-verify");
    $.log(`Finished ${i + 1} of ${dependencyCrates.length} crates.`);
  }

  await cliCrate.publish();
} finally {
  // system beep to notify error or completion
  console.log("\x07");
}
