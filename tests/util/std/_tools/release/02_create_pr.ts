#!/usr/bin/env -S deno run --allow-read --allow-write --allow-env --allow-net --allow-run=git --no-check
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { createOctoKit, getGitHubRepository } from "./deps.ts";
import { loadRepo, VersionFile } from "./repo.ts";

const octoKit = createOctoKit();
const repo = await loadRepo();
const newVersion = new VersionFile().version.toString();

const originalBranch = await repo.gitCurrentBranch();
const newBranchName = `release_${newVersion.replace(/\./, "_")}`;

// Create and push branch
console.log(`Creating branch ${newBranchName}...`);
await repo.gitBranch(newBranchName);
await repo.gitAdd();
await repo.gitCommit(newVersion);
console.log("Pushing branch...");
await repo.gitPush("-u", "origin", "HEAD");

// Open PR
console.log("Opening PR...");
const openedPr = await octoKit.request("POST /repos/{owner}/{repo}/pulls", {
  ...getGitHubRepository(),
  base: originalBranch,
  head: newBranchName,
  draft: true,
  title: newVersion,
  body: getPrBody(),
});
console.log(`Opened PR at ${openedPr.data.url}`);

function getPrBody() {
  let text = `Bumped version for ${newVersion}\n\n` +
    `Please ensure:\n` +
    `- [ ] Version in version.ts is updated correctly\n` +
    `- [ ] Releases.md is updated correctly\n` +
    `- [ ] All the tests in this branch have been run against the CLI release being done\n` +
    "     ```shell\n" +
    `     ../deno/target/release/deno task test\n` +
    "     ```\n" +
    `To make edits to this PR:\n` +
    "```shell\n" +
    `git fetch upstream ${newBranchName} && git checkout -b ${newBranchName} upstream/${newBranchName}\n` +
    "```\n";

  const actor = Deno.env.get("GH_WORKFLOW_ACTOR");
  if (actor !== undefined) {
    text += `\ncc @${actor}`;
  }

  return text;
}
