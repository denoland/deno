#!/usr/bin/env -S deno run -A --quiet --lock=tools/deno.lock.json
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { $, createOctoKit, semver } from "./deps.ts";

const currentDirPath = $.path.dirname($.path.fromFileUrl(import.meta.url));

$.logStep("Getting next version...");
const nextVersion = getNextVersion(semver.parse(getCliVersion())!);

$.logStep("Creating gist with instructions...");
const octoKit = createOctoKit();
const result = await octoKit.request("POST /gists", {
  description: `Deno CLI v${nextVersion} release checklist`,
  public: false,
  files: {
    [`release_${nextVersion}.md`]: {
      content: buildDenoReleaseInstructionsDoc(),
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

function getNextVersion(originalVersion: semver.SemVer) {
  if (Deno.args.some((a) => a === "--patch")) {
    return originalVersion.inc("patch");
  } else if (Deno.args.some((a) => a === "--minor")) {
    return originalVersion.inc("minor");
  } else if (Deno.args.some((a) => a === "--major")) {
    return originalVersion.inc("major");
  } else {
    throw new Error("Missing argument");
  }
}

function buildDenoReleaseInstructionsDoc() {
  const templateText = Deno.readTextFileSync(
    $.path.join(currentDirPath, "release_doc_template.md"),
  );
  return `# Deno CLI ${nextVersion.toString()} Release Checklist\n\n${templateText}`;
}

function getCliVersion() {
  const cargoTomlText = Deno.readTextFileSync(
    $.path.join(currentDirPath, "../../cli/Cargo.toml"),
  );
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
