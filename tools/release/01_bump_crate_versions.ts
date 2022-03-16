#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";
import { GitLogOutput, path, semver } from "./deps.ts";

const workspace = await DenoWorkspace.load();
const repo = workspace.repo;
const cliCrate = workspace.getCliCrate();
const originalCliVersion = cliCrate.version;

// increment the cli version
await cliCrate.promptAndIncrement();

// increment the dependency crate versions
for (const crate of workspace.getCliDependencyCrates()) {
  await crate.increment("minor");
}

// update the lock file
await workspace.getCliCrate().cargoCheck();

// try to update the Releases.md markdown text
try {
  await updateReleasesMd();
} catch (err) {
  console.error(err);
  console.error(
    "Updating Releases.md failed. Please manually run " +
      "`git log --oneline VERSION_FROM..VERSION_TO` and " +
      "use the output to update Releases.md",
  );
}

async function updateReleasesMd() {
  const filePath = path.join(DenoWorkspace.rootDirPath, "Releases.md");
  const oldFileText = await Deno.readTextFile(filePath);
  const insertText = await getReleasesMdText();

  await Deno.writeTextFile(
    filePath,
    oldFileText.replace(/^### /m, insertText + "\n\n### "),
  );

  await workspace.runFormatter();
  console.log(
    "Updated Release.md -- Please review the output to ensure it's correct.",
  );
}

async function getReleasesMdText() {
  const gitLog = await getGitLog();
  const formattedGitLog = gitLog.formatForReleaseMarkdown();
  const formattedDate = getFormattedDate(new Date());

  return `### ${cliCrate.version} / ${formattedDate}\n\n` +
    `${formattedGitLog}`;

  function getFormattedDate(date: Date) {
    const formattedMonth = padTwoDigit(date.getMonth() + 1);
    const formattedDay = padTwoDigit(date.getDate());
    return `${date.getFullYear()}.${formattedMonth}.${formattedDay}`;

    function padTwoDigit(val: number) {
      return val.toString().padStart(2, "0");
    }
  }
}

async function getGitLog() {
  const lastVersion = semver.parse(originalCliVersion)!;
  const lastVersionTag = `v${originalCliVersion}`;
  // fetch the upstream tags
  await repo.gitFetchTags("upstream");

  // this means we're on the patch release
  const latestTag = await repo.gitLatestTag();
  if (latestTag === lastVersionTag) {
    return await repo.getGitLogFromTags(
      "upstream",
      lastVersionTag,
      undefined,
    );
  } else {
    // otherwise, get the history of the last release
    await repo.gitFetchHistory("upstream");
    const lastMinorHistory = await repo.getGitLogFromTags(
      "upstream",
      `v${lastVersion.major}.${lastVersion.minor}.0`,
      lastVersionTag,
    );
    const currentHistory = await repo.getGitLogFromTags(
      "upstream",
      latestTag,
      undefined,
    );
    const lastMinorMessages = new Set(
      lastMinorHistory.lines.map((r) => r.message),
    );
    return new GitLogOutput(
      currentHistory.lines.filter((l) => !lastMinorMessages.has(l.message)),
    );
  }
}
