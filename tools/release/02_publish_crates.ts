#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo --allow-net=crates.io
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";
import { getCratesPublishOrder } from "./deps.ts";

const workspace = await DenoWorkspace.load();
const cliCrate = workspace.getCliCrate();

const dependencyCrates = getCratesPublishOrder(
  workspace.getCliDependencyCrates(),
);

try {
  for (const [i, crate] of dependencyCrates.entries()) {
    await crate.publish();
    console.log(`Published ${i + 1} of ${dependencyCrates.length} crates.`);
  }

  await cliCrate.publish();
} finally {
  // system beep to notify error or completion
  console.log("\x07");
}
