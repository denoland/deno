#!/usr/bin/env -S deno run -A
// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import { patchver } from "jsr:@deno/patchver@0.4.0";

const CHANNEL = Deno.args[0];
if (CHANNEL !== "rc" && CHANNEL !== "lts") {
  throw new Error(`Invalid channel: ${CHANNEL}`);
}

const BINARIES = ["deno.exe", "denort.exe"];

async function patchBinary(inputPath: string, channel: string) {
  console.log(`Patching ${inputPath}...`);

  const input = await Deno.readFile(inputPath);
  const output = patchver(input, channel);

  // Extract filename without extension and create output name
  const baseName = inputPath.replace(/\.exe$/, "");
  const outputPath = `${baseName}-x86_64-pc-windows-msvc-${channel}.exe`;

  await Deno.writeFile(outputPath, output);
  console.log(`Created ${outputPath}`);
}

async function main() {
  for (const binary of BINARIES) {
    await patchBinary(binary, CHANNEL);
  }
  console.log("All Windows binaries patched successfully!");
}

await main();
