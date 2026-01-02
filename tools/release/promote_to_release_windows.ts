#!/usr/bin/env -S deno run -A
// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-console

import { patchver } from "jsr:@deno/patchver@0.5.0";

const CHANNEL = Deno.args[0];
if (CHANNEL !== "rc" && CHANNEL !== "lts") {
  throw new Error(`Invalid channel: ${CHANNEL}`);
}

const BINARIES = ["deno.exe", "denort.exe"];

async function patchBinary(inputPath: string, channel: string) {
  console.log(`Patching ${inputPath}...`);

  await Deno.rename(inputPath, `${inputPath}.bak`);
  const input = await Deno.readFile(`${inputPath}.bak`);
  const output = patchver(input, channel);

  await Deno.writeFile(inputPath, output);
  console.log(`Created ${inputPath}`);
}

async function main() {
  for (const binary of BINARIES) {
    await patchBinary(binary, CHANNEL);
  }
  console.log("All Windows binaries patched successfully!");
}

await main();
