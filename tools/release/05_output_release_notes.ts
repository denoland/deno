#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo,git --no-check
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";

const workspace = await DenoWorkspace.load();

// outputs the release notes for use when creating the GH draft
console.log(
  workspace.getReleasesMdFile().getLatestReleaseText().fullText
    // escape percent signs and newlines in order to allow multiple
    // lines to be set in the GH action step output
    .replace(/%/g, "%25")
    .replace(/\r?\n/g, "%0A"),
);
