#!/usr/bin/env -S deno run -A --quiet --lock=tools/deno.lock.json
// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import { $, createOctoKit, semver } from "./deps.ts";

const currentDirPath = $.path(import.meta.dirname!);

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
    description: `Deno CLI v${semver.format(nextVersion)} release checklist`,
    public: false,
    files: {
      [`release_${semver.format(nextVersion)}.md`]: {
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
    return semver.increment(originalVersion, "patch");
  } else if (Deno.args.some((a) => a === "--minor")) {
    return semver.increment(originalVersion, "minor");
  } else if (Deno.args.some((a) => a === "--major")) {
    return semver.increment(originalVersion, "major");
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
    .replaceAll("$VERSION", semver.format(nextVersion))
    .replaceAll("$MINOR_VERSION", getMinorVersion(semver.format(nextVersion)))
    .replaceAll("$PAST_VERSION", semver.format(currentVersion));
  return `# Deno CLI ${
    semver.format(nextVersion)
  } Release Checklist\n\n${templateText}`;
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
