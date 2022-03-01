#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo,git
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";

const workspace = await DenoWorkspace.load();
const repo = workspace.repo;

const cliCrate = workspace.getCliCrate();
const originalVersion = cliCrate.version;

// increment the version
await cliCrate.promptAndIncrement();
// update the lock file
await cliCrate.cargoCheck();

// output the Releases.md markdown text
console.log(
  "You may use the following as a template for updating Releases.md:\n",
);
console.log(await getReleasesMdText());

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
