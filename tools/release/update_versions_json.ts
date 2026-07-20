#!/usr/bin/env -S deno run -A --lock=tools/deno.lock.json
// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

// Fetches the current versions.json from the bucket, inserts the given release
// version into the "cli" list in descending (newest-first) order (if not
// already present), and writes the result to versions.json.

import { greaterThan, parse } from "jsr:@std/semver@1";

const version = Deno.args[0];
if (!version) {
  console.error("Usage: update_versions_json.ts <version-tag>");
  Deno.exit(1);
}

interface Versions {
  cli: string[];
}

let versions: Versions = { cli: [] };
const response = await fetch("https://dl.deno.land/versions.json");
if (response.ok) {
  versions = await response.json();
  if (!Array.isArray(versions.cli)) {
    versions.cli = [];
  }
} else if (response.status !== 404) {
  throw new Error(`Failed to fetch versions.json: ${response.statusText}`);
} else {
  // Ensure the body is consumed so the connection can be reused/closed.
  await response.body?.cancel();
}

// Insert in descending (newest-first) order rather than blindly prepending.
// Releases are not always published newest-first: an LTS patch (e.g. v2.9.5)
// can ship after a newer stable (e.g. v3.0.0), and a prepend would then list it
// above the newer versions. Insert by semver so the ordering stays correct.
if (!versions.cli.includes(version)) {
  const parsed = parse(version);
  const idx = versions.cli.findIndex((v) => greaterThan(parsed, parse(v)));
  if (idx === -1) {
    versions.cli.push(version);
  } else {
    versions.cli.splice(idx, 0, version);
  }
}

console.error("Adding version:", version);
console.error("Total versions:", versions.cli.length);

Deno.writeTextFileSync("versions.json", JSON.stringify(versions) + "\n");
