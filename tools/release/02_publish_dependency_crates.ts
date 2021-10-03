#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo --allow-net=crates.io
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace, getCratesPublishOrder } from "./helpers/mod.ts";

const workspace = await DenoWorkspace.load();

const dependencyCrates = getCratesPublishOrder(workspace.getDependencyCrates());

try {
  for (const [i, crate] of dependencyCrates.entries()) {
    await crate.publish();
    console.log(`Published ${i + 1} of ${dependencyCrates.length} crates.`);
  }
} finally {
  // system beep to notify error or completion
  console.log("\x07");
}
