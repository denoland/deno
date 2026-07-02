#!/usr/bin/env -S deno run -A --lock=tools/deno.lock.json
// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

// Fetches the current versions.json from the bucket, prepends the given
// release version to the "cli" list (if not already present), writes the
// result to versions.json and prints the file name to stdout.

const version = Deno.args[0];
if (!version) {
  console.error("Usage: update_versions_json.ts <version-tag>");
  Deno.exit(1);
}

const outFile = "versions.json";

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

// The freshly published release is always the newest, so prepend it.
if (!versions.cli.includes(version)) {
  versions.cli.unshift(version);
}

console.error("Adding version:", version);
console.error("Total versions:", versions.cli.length);

Deno.writeTextFileSync(outFile, JSON.stringify(versions) + "\n");

// Print the file name so the caller knows what to upload.
console.log(outFile);
