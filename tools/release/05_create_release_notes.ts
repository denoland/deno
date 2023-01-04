#!/usr/bin/env -S deno run -A --lock=tools/deno.lock.json
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { $ } from "./deps.ts";
import { DenoWorkspace } from "./deno_workspace.ts";

const workspace = await DenoWorkspace.load();

// create a release notes file for the GH release draft
await Deno.writeTextFile(
  $.path.join(DenoWorkspace.rootDirPath, "./target/release/release-notes.md"),
  workspace.getReleasesMdFile().getLatestReleaseText().fullText,
);
