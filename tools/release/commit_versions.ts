#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";

const workspace = await DenoWorkspace.load();
const repo = workspace.repo;
const cliCrate = workspace.getCliCrate();

const branchName = `release_${cliCrate.version.replace(/\./, "_")}`;

await repo.gitBranch(branchName);
await repo.gitAdd();
await repo.gitCommit(cliCrate.version);
await repo.gitPush();
