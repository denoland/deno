#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo,git --no-check
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";

const workspace = await DenoWorkspace.load();
const repo = workspace.repo;
const cliCrate = workspace.getCliCrate();

await repo.gitFetchTags("origin");
const tags = await repo.getGitTags();
const tagName = `v${cliCrate.version}`;

if (tags.has(tagName)) {
  console.log(`Tag ${tagName} already exists.`);
} else {
  await repo.gitTag(tagName);
  await repo.gitPush(tagName);
}
