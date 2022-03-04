#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";

const workspace = await DenoWorkspace.load();

for (const crate of workspace.getDependencyCrates()) {
  await crate.increment("minor");
}

// update the lock file
await workspace.getCliCrate().cargoCheck();
