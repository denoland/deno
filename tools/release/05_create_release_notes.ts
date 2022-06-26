#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo,git --no-check --lock=tools/deno.lock.json
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { path } from "./deps.ts";
import { DenoWorkspace } from "./deno_workspace.ts";

const workspace = await DenoWorkspace.load();

// create a release notes file for the GH release draft
await Deno.writeTextFile(
  path.join(DenoWorkspace.rootDirPath, "./target/release/release-notes.md"),
  workspace.getReleasesMdFile().getLatestReleaseText().fullText,
);
