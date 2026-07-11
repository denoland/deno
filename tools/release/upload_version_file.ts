#!/usr/bin/env -S deno run -A --lock=tools/deno.lock.json
// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

// Determines the correct latest-version file for a given release tag,
// writes the version into it, and prints the file name to stdout.
//
// Pass `--lts` (or set DENO_LTS) for an LTS release: it targets
// `release-lts-latest.txt` and never touches the stable pointer, so an LTS
// patch (a plain version like 2.9.4) can't be mistaken for the latest stable.
//
// For stable and LTS releases, exits with code 1 if the version is not greater
// than the currently published latest for that channel (to skip the upload).

import { greaterThan, parse } from "jsr:@std/semver@1";

const isLts = Deno.args.includes("--lts") ||
  Boolean(Deno.env.get("DENO_LTS"));
const version = Deno.args.find((a) => !a.startsWith("--"));
if (!version) {
  console.error("Usage: upload_version_file.ts <version-tag> [--lts]");
  Deno.exit(1);
}

let latestFile: string;
if (isLts) {
  // LTS versions are plain semver, so this must take precedence over the
  // suffix checks below and never fall through to `release-latest.txt`.
  latestFile = "release-lts-latest.txt";
} else if (version.includes("-alpha.")) {
  latestFile = "release-alpha-latest.txt";
} else if (version.includes("-beta.")) {
  latestFile = "release-beta-latest.txt";
} else {
  latestFile = "release-latest.txt";
}

console.error("Version:", version);
console.error("Latest file:", latestFile);

Deno.writeTextFileSync(latestFile, version + "\n");

// Stable and LTS pointers must never regress.
if (
  latestFile === "release-latest.txt" ||
  latestFile === "release-lts-latest.txt"
) {
  const response = await fetch(`https://dl.deno.land/${latestFile}`);
  if (response.ok) {
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
  } else if (response.status !== 404) {
    // 404 means the channel has no pointer yet (first release) -> allow.
    throw new Error(`Failed to fetch: ${response.statusText}`);
  }
}

// Print the file name so the caller knows what to upload
console.log(latestFile);
