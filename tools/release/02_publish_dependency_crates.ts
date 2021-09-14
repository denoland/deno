#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo --allow-net=crates.io
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace, getCratesPublishOrder } from "./helpers/mod.ts";

const workspace = await DenoWorkspace.load();

const dependencyCrates = workspace.getDependencyCrates();

for (const crate of getCratesPublishOrder(dependencyCrates)) {
  await crate.publish();
}
