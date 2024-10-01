#!/usr/bin/env -S deno run -A --lock=tools/deno.lock.json
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-console

import { $ } from "jsr:@david/dax@0.41.0";
import { gray } from "jsr:@std/fmt@1/colors";
import { patchver } from "jsr:@deno/patchver@0.2.0";

const SUPPORTED_TARGETS = [
  "aarch64-apple-darwin",
  "aarch64-unknown-linux-gnu",
  "x86_64-apple-darwin",
  "x86_64-pc-windows-msvc",
  "x86_64-unknown-linux-gnu",
];

const DENO_BINARIES = [
  "deno",
  "denort",
];

const CHANNEL = Deno.args[0];
if (CHANNEL !== "rc" && CHANNEL !== "lts") {
  throw new Error(`Invalid channel: ${CHANNEL}`);
}

const CANARY_URL = "https://dl.deno.land";

function getCanaryBinaryUrl(
  version: string,
  binary: string,
  target: string,
): string {
  return `${CANARY_URL}/canary/${version}/${binary}-${target}.zip`;
}

function getUnzippedFilename(binary: string, target: string) {
  if (target.includes("windows")) {
    return `${binary}.exe`;
  } else {
    return binary;
  }
}

function getBinaryName(binary: string, target: string): string {
  let ext = "";
  if (target.includes("windows")) {
    ext = ".exe";
  }
  return `${binary}-${target}-${CHANNEL}${ext}`;
}

function getArchiveName(binary: string, target: string): string {
  return `${binary}-${target}.zip`;
}

interface CanaryVersion {
  target: string;
  version: string;
}

async function remove(filePath: string) {
  try {
    await Deno.remove(filePath);
  } catch {
    // pass
  }
}

async function fetchLatestCanaryBinary(
  version: string,
  binary: string,
  target: string,
) {
  const url = getCanaryBinaryUrl(version, binary, target);
  await $.request(url).showProgress().pipeToPath();
}

async function fetchLatestCanaryBinaries(canaryVersion: string) {
  for (const binary of DENO_BINARIES) {
    for (const target of SUPPORTED_TARGETS) {
      $.logStep("Download", binary, gray("target:"), target);
      await fetchLatestCanaryBinary(canaryVersion, binary, target);
    }
  }
}

async function unzipArchive(archiveName: string, unzippedName: string) {
  await remove(unzippedName);
  const output = await $`unzip ./${archiveName}`;
  if (output.code !== 0) {
    $.logError(`Failed to unzip ${archiveName} (error code ${output.code})`);
    Deno.exit(1);
  }
}

async function createArchive(binaryName: string, archiveName: string) {
  const output = await $`zip -r ./${archiveName} ./${binaryName}`;

  if (output.code !== 0) {
    $.logError(
      `Failed to create archive ${archiveName} (error code ${output.code})`,
    );
    Deno.exit(1);
  }
}

async function runPatchver(
  binary: string,
  target: string,
  binaryName: string,
) {
  const input = await Deno.readFile(binary);
  const output = patchver(input, CHANNEL);

  try {
    await Deno.writeFile(binaryName, output);
  } catch (e) {
    $.logError(
      `Failed to promote to RC ${binary} (${target}), error:`,
      e,
    );
    Deno.exit(1);
  }
}

async function runRcodesign(
  target: string,
  binaryName: string,
  commitHash: string,
) {
  if (!target.includes("apple") || binaryName.includes("denort")) {
    return;
  }
  $.logStep(`Codesign ${binaryName}`);
  const tempFile = $.path("temp.p12");
  let output;
  try {
    await $`echo $APPLE_CODESIGN_KEY | base64 -d`.stdout(tempFile);
    output =
      await $`rcodesign sign ./${binaryName} --binary-identifier=deno-${commitHash} --code-signature-flags=runtime --code-signature-flags=runtime --p12-password="$APPLE_CODESIGN_PASSWORD" --p12-file=${tempFile} --entitlements-xml-file=cli/entitlements.plist`;
  } finally {
    try {
      tempFile.removeSync();
    } catch {
      // pass
    }
  }
  if (output.code !== 0) {
    $.logError(
      `Failed to codesign ${binaryName} (error code ${output.code})`,
    );
    Deno.exit(1);
  }
  await $`codesign -dv --verbose=4 ./deno`;
}

async function promoteBinaryToRc(
  binary: string,
  target: string,
  commitHash: string,
) {
  const unzippedName = getUnzippedFilename(binary, target);
  const binaryName = getBinaryName(binary, target);
  const archiveName = getArchiveName(binary, target);
  await remove(unzippedName);
  await remove(binaryName);
  $.logStep(
    "Unzip",
    archiveName,
    gray("binary"),
    binary,
    gray("binaryName"),
    binaryName,
  );

  await unzipArchive(archiveName, unzippedName);
  await remove(archiveName);

  $.logStep(
    "Patchver",
    unzippedName,
    `(${target})`,
    gray("output to"),
    binaryName,
  );
  await runPatchver(unzippedName, target, binaryName);
  // Remove the unpatched binary and rename patched one.
  await remove(unzippedName);
  await Deno.rename(binaryName, unzippedName);
  await runRcodesign(target, unzippedName, commitHash);
  // Set executable permission
  if (!target.includes("windows")) {
    Deno.chmod(unzippedName, 0o777);
  }

  await createArchive(unzippedName, archiveName);
  await remove(unzippedName);
}

async function promoteBinariesToRc(commitHash: string) {
  const totalCanaries = SUPPORTED_TARGETS.length * DENO_BINARIES.length;

  for (let targetIdx = 0; targetIdx < SUPPORTED_TARGETS.length; targetIdx++) {
    const target = SUPPORTED_TARGETS[targetIdx];
    for (let binaryIdx = 0; binaryIdx < DENO_BINARIES.length; binaryIdx++) {
      const binaryName = DENO_BINARIES[binaryIdx];
      const currentIdx = (targetIdx * 2) + binaryIdx + 1;
      $.logLight(
        `[${currentIdx}/${totalCanaries}]`,
        "Promote",
        binaryName,
        target,
        `to ${CHANNEL}...`,
      );
      await promoteBinaryToRc(binaryName, target, commitHash);
      $.logLight(
        `[${currentIdx}/${totalCanaries}]`,
        "Promoted",
        binaryName,
        target,
        `to ${CHANNEL}!`,
      );
    }
  }
}

async function dumpRcVersion() {
  $.logStep("Compute version");
  await unzipArchive(getArchiveName("deno", Deno.build.target), "deno");
  const output = await $`./deno -V`.stdout("piped");
  const denoVersion = output.stdout.slice(5).split("+")[0];
  $.logStep("Computed version", denoVersion);
  await Deno.writeTextFile(
    `./release-${CHANNEL}-latest.txt`,
    `v${denoVersion}`,
  );
}

async function main() {
  const commitHash = Deno.args[1];
  if (!commitHash) {
    throw new Error("Commit hash needs to be provided as an argument");
  }
  $.logStep("Download canary binaries...");
  await fetchLatestCanaryBinaries(commitHash);
  console.log("All canary binaries ready");
  $.logStep(`Promote canary binaries to ${CHANNEL}...`);
  await promoteBinariesToRc(commitHash);

  // Finally dump the version name to a `release.txt` file for uploading to GCP
  await dumpRcVersion();
}

await main();
