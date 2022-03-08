#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo,git
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";
import { path } from "./deps.ts";

const workspace = await DenoWorkspace.load();
const repo = workspace.repo;

const cliCrate = workspace.getCliCrate();
const originalVersion = cliCrate.version;

// increment the version
await cliCrate.promptAndIncrement();
// update the lock file
await cliCrate.cargoCheck();

// update the Releases.md markdown text
await updateReleasesMd();
await workspace.runFormatter();
console.log(
  "Updated Release.md -- Please review the output to ensure it's correct.",
);

async function updateReleasesMd() {
  const filePath = path.join(DenoWorkspace.rootDirPath, "Releases.md");
  const oldFileText = await Deno.readTextFile(filePath);
  const insertText = await getReleasesMdText();

  await Deno.writeTextFile(
    filePath,
    oldFileText.replace(/^### /m, insertText + "\n\n### "),
  );
}

async function getReleasesMdText() {
  const gitLog = await repo.getGitLogFromTags(
    "upstream",
    `v${originalVersion}`,
    undefined,
  );
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
