#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo --no-check
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";

const workspace = await DenoWorkspace.load();
const repo = workspace.repo;
const cliCrate = workspace.getCliCrate();

await repo.gitFetchTags("origin");
const tags = await repo.getGitTags();

if (tags.has(cliCrate.version)) {
  console.log(`Tag ${cliCrate.version} already exists.`);
} else {
  await repo.gitTag(cliCrate.version);
  await repo.gitPush(cliCrate.version);
}
