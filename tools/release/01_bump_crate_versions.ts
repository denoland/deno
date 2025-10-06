#!/usr/bin/env -S deno run -A --lock=tools/deno.lock.json
// Copyright 2018-2025 the Deno authors. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";
import { $, GitLogOutput, semver } from "./deps.ts";

const workspace = await DenoWorkspace.load();
const repo = workspace.repo;
const cliCrate = workspace.getCliCrate();
const denoRtCrate = workspace.getDenoRtCrate();
const denoLibCrate = workspace.getDenoLibCrate();
const originalCliVersion = cliCrate.version;

if (Deno.args.some((a) => a === "--rc")) {
  let cliVersion = semver.parse(cliCrate.version)!;

  if (cliVersion.prerelease?.[0] != "rc") {
    cliVersion = semver.increment(cliVersion, "minor");
  }
  cliVersion = increment(cliVersion, "prerelease", { prerelease: "rc" });

  const version = cliVersion.toString();

  await cliCrate.setVersion(version);
  await denoRtCrate.setVersion(version);
  denoLibCrate.folderPath.join("version.txt").writeTextSync(version);
  // Force lockfile update
  await workspace.getCliCrate().cargoUpdate("--workspace");

  await assertDenoBinaryVersion(version);

  Deno.exit(0);
}

await bumpCiCacheVersion();

// increment the cli version
if (Deno.args.some((a) => a === "--patch")) {
  await cliCrate.increment("patch");
} else if (Deno.args.some((a) => a === "--minor")) {
  await cliCrate.increment("minor");
} else if (Deno.args.some((a) => a === "--major")) {
  await cliCrate.increment("major");
} else {
  await cliCrate.promptAndIncrement();
}

await denoRtCrate.setVersion(cliCrate.version);
denoLibCrate.folderPath.join("version.txt").writeTextSync(cliCrate.version);

// increment the dependency crate versions
for (const crate of workspace.getCliDependencyCrates()) {
  await crate.increment("minor");
}

// update the lock file
await workspace.getCliCrate().cargoUpdate("--workspace");
await assertDenoBinaryVersion(cliCrate.version);

// try to update the Releases.md markdown text
try {
  $.logStep("Updating Releases.md...");
  await updateReleasesMd();
} catch (err) {
  $.log(err);
  $.logError(
    "Error Updating Releases.md failed. Please manually run " +
      "`git log --oneline VERSION_FROM..VERSION_TO` and " +
      "use the output to update Releases.md",
  );
}

async function updateReleasesMd() {
  const gitLog = await getGitLog();
  const releasesMdFile = workspace.getReleasesMdFile();
  const cliVersion = semver.parse(cliCrate.version)!;
  const bodyPreText = releaseHasBlogPost()
    ? `Read more: http://deno.com/blog/v${cliVersion.major}.${cliVersion.minor}`
    : undefined;
  releasesMdFile.updateWithGitLog({
    version: cliCrate.version,
    gitLog,
    bodyPreText,
  });

  await workspace.runFormatter();
}

async function getGitLog() {
  const originalVersion = semver.parse(originalCliVersion)!;
  const originalVersionTag = `v${originalCliVersion}`;
  // fetch the upstream tags
  await repo.gitFetchTags("upstream");

  // make the repo unshallow so we can fetch the latest tag
  if (await repo.gitIsShallow()) {
    await repo.gitFetchUnshallow("origin");
  }

  // this means we're on the patch release
  const latestTag = await repo.gitLatestTag();
  if (latestTag === originalVersionTag) {
    return await repo.getGitLogFromTags(
      "upstream",
      originalVersionTag,
      undefined,
    );
  } else {
    // otherwise, get the history of the last release
    await repo.gitFetchHistory("upstream");
    const lastMinorHistory = await repo.getGitLogFromTags(
      "upstream",
      `v${originalVersion.major}.${originalVersion.minor}.0`,
      originalVersionTag,
    );
    const currentHistory = await repo.getGitLogFromTags(
      "upstream",
      latestTag,
      undefined,
    );
    const lastMinorMessages = new Set(
      lastMinorHistory.lines.map((r) => r.message),
    );
    return new GitLogOutput(
      currentHistory.lines.filter((l) => !lastMinorMessages.has(l.message)),
    );
  }
}

async function bumpCiCacheVersion() {
  const generateScript = workspace.repo.folderPath.join(
    ".github/workflows/ci.generate.ts",
  );
  const fileText = generateScript.readTextSync();
  const cacheVersionRegex = /const cacheVersion = ([0-9]+);/;
  const version = fileText.match(cacheVersionRegex)?.[1];
  if (version == null) {
    throw new Error("Could not find cache version in text.");
  }
  const toVersion = parseInt(version, 10) + 1;
  $.logStep(`Bumping cache version from ${version} to ${toVersion}...`);
  const newText = fileText.replace(
    cacheVersionRegex,
    `const cacheVersion = ${toVersion};`,
  );
  generateScript.writeTextSync(newText);

  // run the script
  await $`${generateScript}`;
}

async function assertDenoBinaryVersion(expectedVersion: string) {
  $.logStep("Verifying Deno binary version.");
  const text = (await $`cargo run -p deno -- -v`.text()).replace("deno ", "");
  $.logLight("Version:", text);
  if (text.trim() !== expectedVersion) {
    $.logError("Error: Expected", expectedVersion, "but found", text);
    Deno.exit(1);
  }
}

function releaseHasBlogPost() {
  const pastVersion = semver.parse(originalCliVersion)!;
  const newVersion = semver.parse(cliCrate.version)!;

  return pastVersion.major !== newVersion.major ||
    pastVersion.minor !== newVersion.minor;
}
