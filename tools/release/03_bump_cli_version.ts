#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo,git
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  DenoWorkspace,
  formatGitLogForMarkdown,
  getGitLogFromTag,
} from "./helpers/mod.ts";

const workspace = await DenoWorkspace.load();

const cliCrate = workspace.getCliCrate();
const originalVersion = cliCrate.version;

// increment the version
await cliCrate.increment(getVersionIncrement());
await workspace.updateLockFile();

// output the Releases.md markdown text
console.log(
  "You may use the following as a template for updating Releases.md:\n",
);
console.log(await getReleasesMdText());

function getVersionIncrement() {
  if (confirm("Increment patch?")) {
    return "patch";
  } else if (confirm("Increment minor?")) {
    return "minor";
  } else if (confirm("Increment major?")) {
    return "major";
  } else {
    throw new Error("No decision.");
  }
}

async function getReleasesMdText() {
  const gitLogOutput = await getGitLogFromTag(
    DenoWorkspace.rootDirPath,
    `v${originalVersion}`,
  );
  const formattedGitLog = formatGitLogForMarkdown(gitLogOutput);
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
