#!/usr/bin/env -S deno run -A --lock=tools/deno.lock.json
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";
import { $, createOctoKit, getGitHubRepository } from "./deps.ts";

const octoKit = createOctoKit();
const workspace = await DenoWorkspace.load();
const repo = workspace.repo;
const cliCrate = workspace.getCliCrate();

const originalBranch = await repo.gitCurrentBranch();
const newBranchName = `release_${cliCrate.version.replace(/\./, "_")}`;

// Create and push branch
$.logStep(`Creating branch ${newBranchName}...`);
await repo.gitBranch(newBranchName);
await repo.gitAdd();
await repo.gitCommit(cliCrate.version);
$.logStep("Pushing branch...");
await repo.gitPush("-u", "origin", "HEAD");

// Open PR
$.logStep("Opening PR...");
const openedPr = await octoKit.request("POST /repos/{owner}/{repo}/pulls", {
  ...getGitHubRepository(),
  base: originalBranch,
  head: newBranchName,
  draft: true,
  title: cliCrate.version,
  body: getPrBody(),
});
$.log(`Opened PR at ${openedPr.data.url}`);

function getPrBody() {
  let text = `Bumped versions for ${cliCrate.version}\n\n` +
    `Please ensure:\n` +
    `- [ ] Target branch is correct\n` +
    `- [ ] Crate versions are bumped correctly\n` +
    `- [ ] deno_std version is incremented in the code (see \`cli/deno_std.rs\`)\n` +
    `- [ ] Releases.md is updated correctly\n\n` +
    `To make edits to this PR:\n` +
    "```shell\n" +
    `git fetch upstream ${newBranchName} && git checkout -b ${newBranchName} upstream/${newBranchName}\n` +
    "```\n";

  const actor = Deno.env.get("GH_WORKFLOW_ACTOR");
  if (actor != null) {
    text += `\ncc @${actor}`;
  }

  return text;
}
