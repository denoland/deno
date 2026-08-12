#!/usr/bin/env -S deno run -A --lock=tools/deno.lock.json
// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

// Determines the correct latest-version file for a given release tag,
// writes the version into it, and prints the file name to stdout.
// For stable releases, exits with code 1 if the version is not greater
// than the current published latest (to skip the upload).

import { greaterThan, parse } from "jsr:@std/semver@1";

const version = Deno.args[0];
if (!version) {
  console.error("Usage: upload_version_file.ts <version-tag>");
  Deno.exit(1);
}

let latestFile: string;
if (version.includes("-alpha.")) {
  latestFile = "release-alpha-latest.txt";
} else if (version.includes("-beta.")) {
  latestFile = "release-beta-latest.txt";
} else {
  latestFile = "release-latest.txt";
}

console.error("Version:", version);
console.error("Latest file:", latestFile);

Deno.writeTextFileSync(latestFile, version + "\n");

// For stable releases, check that this version is actually newer
if (latestFile === "release-latest.txt") {
  const response = await fetch("https://dl.deno.land/release-latest.txt");
  if (!response.ok) {
    throw new Error(`Failed to fetch: ${response.statusText}`);
  }
  const latestVersionText = (await response.text()).trim();
  console.error("Currently published latest:", latestVersionText);

  const latestVersion = parse(latestVersionText);
  const currentVersion = parse(version);
  if (!greaterThan(currentVersion, latestVersion)) {
    console.error(
      "Skipping upload because this version is not greater than the latest.",
    );
    Deno.exit(1);
  }
}

// Print the file name so the caller knows what to upload
console.log(latestFile);
