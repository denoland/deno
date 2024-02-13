#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=git,deno --no-check
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { getReleasesMdFile, loadRepo, VersionFile } from "./repo.ts";

const repo = await loadRepo();
const versionFile = new VersionFile();
const currentVersion = versionFile.version;
const newVersion = versionFile.version.inc(getVersionIncrementKind());

// update version.ts
console.log(`Bumping version from ${currentVersion} to ${newVersion}...`);
versionFile.updateVersion(newVersion);

// update Releases.md
const releasesMdFile = getReleasesMdFile();
const versionGitLog = await getGitLogFromLastVersion();
releasesMdFile.updateWithGitLog({
  gitLog: versionGitLog,
  version: newVersion.toString(),
});

// run deno fmt
await repo.runCommandWithOutput(["deno", "fmt", "Releases.md"]);

function getVersionIncrementKind() {
  if (Deno.args.some((a) => a === "--patch")) {
    return "patch";
  } else if (Deno.args.some((a) => a === "--minor")) {
    return "minor";
  } else if (Deno.args.some((a) => a === "--major")) {
    return "major";
  } else {
    throw new Error("Please provide a --patch, --minor, or --major flag.");
  }
}

async function getGitLogFromLastVersion() {
  // fetch the upstream tags and history
  await repo.gitFetchTags("upstream");
  await repo.gitFetchHistory("upstream");

  // get the git log from the current commit to the last version
  return await repo.getGitLogFromTags(
    "upstream",
    currentVersion.toString(),
    undefined,
  );
}
