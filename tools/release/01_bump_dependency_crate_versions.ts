#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./helpers/mod.ts";

const workspace = await DenoWorkspace.load();

for (const crate of workspace.getDependencyCrates()) {
  await crate.increment("minor");
}
