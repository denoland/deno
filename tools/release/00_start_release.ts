#!/usr/bin/env -S deno run -A --quiet --lock=tools/deno.lock.json
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-console

import { $, createOctoKit, semver } from "./deps.ts";

const currentDirPath = $.path(import.meta).parentOrThrow();

$.logStep("Getting next version...");
const currentVersion = semver.parse(getCliVersion())!;
const nextVersion = getNextVersion(semver.parse(getCliVersion())!);

$.logStep("Creating gist with instructions...");
const releaseInstructions = buildDenoReleaseInstructionsDoc();
if (Deno.args.some((a) => a === "--dry-run")) {
  console.log(releaseInstructions);
} else {
  const octoKit = createOctoKit();
  const result = await octoKit.request("POST /gists", {
    description: `Deno CLI v${nextVersion} release checklist`,
    public: false,
    files: {
      [`release_${nextVersion}.md`]: {
        content: releaseInstructions,
      },
    },
  });

  $.log("==============================================");
  $.log("Created gist with instructions!");
  $.log("");
  $.log(`  ${result.data.html_url}`);
  $.log("");
  $.log("Please fork the gist and follow the checklist.");
  $.log("==============================================");
}

function getNextVersion(originalVersion: semver.SemVer) {
  if (Deno.args.some((a) => a === "--patch")) {
    return originalVersion.increment("patch");
  } else if (Deno.args.some((a) => a === "--minor")) {
    return originalVersion.increment("minor");
  } else if (Deno.args.some((a) => a === "--major")) {
    return originalVersion.increment("major");
  } else {
    throw new Error("Missing argument");
  }
}

function buildDenoReleaseInstructionsDoc() {
  function getMinorVersion(version: string) {
    return version.split(".").slice(0, 2).join(".");
  }

  const templateText = currentDirPath
    .join("release_doc_template.md")
    .readTextSync()
    .replaceAll("$BRANCH_NAME", `v${nextVersion.major}.${nextVersion.minor}`)
    .replaceAll("$VERSION", nextVersion.toString())
    .replaceAll("$MINOR_VERSION", getMinorVersion(nextVersion.toString()))
    .replaceAll("$PAST_VERSION", currentVersion.toString());
  return `# Deno CLI ${nextVersion.toString()} Release Checklist\n\n${templateText}`;
}

function getCliVersion() {
  const cargoTomlText = currentDirPath
    .join("../../cli/Cargo.toml")
    .readTextSync();
  const result = cargoTomlText.match(/^version\s*=\s*"([^"]+)"$/m);
  if (result == null || result.length !== 2) {
    $.log("Cargo.toml");
    $.log("==========");
    $.log(cargoTomlText);
    $.log("==========");
    throw new Error("Could not find version in text.");
  }
  return result[1];
}
