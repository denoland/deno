#!/usr/bin/env -S deno run -A --lock=tools/deno.lock.json
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";
import { $, createOctoKit, semver } from "./deps.ts";

$.logStep("Loading cli crate...");
const workspace = await DenoWorkspace.load();
const cliCrate = workspace.getCliCrate();
const nextVersion = getNextVersion(semver.parse(cliCrate.version)!);

$.logStep("Creating gist with instructions...");
const octoKit = createOctoKit();
const result = await octoKit.request("POST /gists", {
  description: `Deno CLI v${nextVersion.toString()} release checklist`,
  public: false,
  files: {
    "release_instructions.md": {
      content: buildDenoReleaseInstructionsDoc(),
    },
  },
});

$.log("==============================================");
$.log("Created gist with instructions!");
$.log("");
$.log(`  ${result.url}`);
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
  const currentDirPath = $.path.dirname($.path.fromFileUrl(import.meta.url));
  const templateText = Deno.readTextFileSync(
    $.path.join(currentDirPath, "release_doc_template.md"),
  );
  return `# Deno CLI ${nextVersion.toString()} Release Checklist\n\n${templateText}`;
}
