// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-console
import { greaterThan, parse } from "jsr:@std/semver@1";

const response = await fetch("https://dl.deno.land/release-latest.txt");
if (!response.ok) {
  throw new Error(`Failed to fetch: ${response.statusText}`);
}

const latestVersionText = (await response.text()).trim();
const currentVersionText = Deno.args[0];
console.error("Latest version:", latestVersionText);
console.error("Current version:", currentVersionText);

const latestVersion = parse(latestVersionText);
const currentVersion = parse(currentVersionText);
const isGreater = greaterThan(currentVersion, latestVersion);
if (isGreater) {
  console.error("Updating release-latest.txt");
} else {
  console.error(
    "Skipping release-latest.txt update because this version is not greater than the latest.",
  );
}
Deno.exit(isGreater ? 0 : 1);
