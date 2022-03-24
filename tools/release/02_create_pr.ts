#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo,git
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";
import { createOctoKit, getGitHubRepository } from "./deps.ts";

const octoKit = createOctoKit();
const workspace = await DenoWorkspace.load();
const repo = workspace.repo;
const cliCrate = workspace.getCliCrate();

const originalBranch = await repo.gitCurrentBranch();
const newBranchName = `release_${cliCrate.version.replace(/\./, "_")}`;

// Create and push branch
console.log(`Creating branch ${newBranchName}...`);
await repo.gitBranch(newBranchName);
await repo.gitAdd();
await repo.gitCommit(cliCrate.version);
console.log("Pushing branch...");
await repo.gitPush("-u", "origin", "HEAD");

// Open PR
console.log("Opening PR...");
const openedPr = await octoKit.request("POST /repos/{owner}/{repo}/pulls", {
  ...getGitHubRepository(),
  base: originalBranch,
  head: newBranchName,
  draft: true,
  title: cliCrate.version,
  body: getPrBody(),
});
console.log(`Opened PR at ${openedPr.data.url}`);

function getPrBody() {
  let text = `Bumped versions for ${cliCrate.version}`;
  const actor = Deno.env.get("GH_WORKFLOW_ACTOR");
  if (actor != null) {
    text += `\n\ncc @${actor}`;
  }
  return text;
}
