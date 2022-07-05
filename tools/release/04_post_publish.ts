#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo,git --allow-net --allow-env --no-check --lock=tools/deno.lock.json
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";
import { createOctoKit, getGitHubRepository } from "./deps.ts";

const workspace = await DenoWorkspace.load();
const repo = workspace.repo;
const cliCrate = workspace.getCliCrate();

console.log("Creating release tag...");
await createReleaseTag();

console.log("Forwarding release commit to main...");
try {
  await forwardReleaseCommitToMain();
} catch (err) {
  console.error("Failed. Please manually open a PR.", err);
}

async function createReleaseTag() {
  await repo.gitFetchTags("origin");
  const tags = await repo.getGitTags();
  const tagName = `v${cliCrate.version}`;

  if (tags.has(tagName)) {
    console.log(`Tag ${tagName} already exists.`);
  } else {
    await repo.gitTag(tagName);
    await repo.gitPush("origin", tagName);
  }
}

async function forwardReleaseCommitToMain() {
  // if this is a patch release, open a PR to forward the most recent commit back to main
  const currentBranch = await repo.gitCurrentBranch();
  const isPatchRelease = currentBranch !== "main";

  if (!isPatchRelease) {
    console.log("Not doing a patch release. Skipping.");
    return;
  }

  await repo.runCommandWithOutput(["git", "fetch", "origin", "main"]);
  const releaseCommitHash =
    (await repo.runCommand(["git", "rev-parse", "HEAD"])).trim();
  const newBranchName = `forward_v${cliCrate.version}`;
  console.log(`Creating branch ${newBranchName}...`);
  await repo.runCommand([
    "git",
    "checkout",
    "-b",
    newBranchName,
    "origin/main",
  ]);
  await repo.runCommand([
    "git",
    "cherry-pick",
    "--strategy-option",
    "theirs",
    releaseCommitHash,
  ]);
  await repo.gitPush("origin", newBranchName);

  console.log(`Opening PR...`);
  const openedPr = await createOctoKit().request(
    "POST /repos/{owner}/{repo}/pulls",
    {
      ...getGitHubRepository(),
      base: "main",
      head: newBranchName,
      draft: true,
      title: `chore: forward v${cliCrate.version} release commit to main`,
      body: getPrBody(),
    },
  );
  console.log(`Opened PR at ${openedPr.data.url}`);

  function getPrBody() {
    let text =
      `This is the release commit being forwarded back to main for ${cliCrate.version}\n\n` +
      `Please ensure:\n` +
      `- [ ] Everything looks ok in the PR\n` +
      `- [ ] The release has been published\n\n` +
      `To make edits to this PR:\n` +
      "```shell\n" +
      `git fetch upstream ${newBranchName} && git checkout -b ${newBranchName} upstream/${newBranchName}\n` +
      "```\n\n" +
      "Don't need this PR? Close it.\n";

    const actor = Deno.env.get("GH_WORKFLOW_ACTOR");
    if (actor != null) {
      text += `\ncc @${actor}`;
    }

    return text;
  }
}
