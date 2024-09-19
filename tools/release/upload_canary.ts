#!/usr/bin/env -S deno run -A --lock=tools/deno.lock.json
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-console

import { $ } from "jsr:@david/dax@0.41.0";

async function gitRevParse() {
  return $`git rev-parse HEAD`;
}

async function uploadCanaryForCurrentArch(currentGitSha) {
  const currentGitRev = await gitRevParse();
  await $`gsutil -h "Cache-Control: public, max-age=3600" cp ./target/release/*.zip gs://dl.deno.land/canary/${currentGitRev}/`;
  const llvmTriple = await $`rustc -vV | sed -n "s|host: ||p"`;
  const latestCanaryHash =
    await $`gsutil cat gs://dl.deno.land/canary-${llvmTriple}-latest.txt`;
  const commitExistsInHistory = await $`git cat-file -e ${latestCanaryHash}`;

  if (commitExistsInHistory.code === 0) {
    await Deno.writeTextFile("canary-latest.txt", currentGitSha);
    await $`gsutil -h "Cache-Control: no-cache" cp canary-latest.txt gs://dl.deno.land/canary-${llvmTriple}-latest.txt`;
  } else {
    console.log("Skipping upload, newer canary version is already available");
  }
}

async function uploadCanaryLatest(currentGitSha) {
  const latestCanaryHash = $`gsutil cat gs://dl.deno.land/canary-latest.txt`;
  const commitExistsInHistory = await $`git cat-file -e ${latestCanaryHash}`;

  if (commitExistsInHistory.code === 0) {
    await Deno.writeTextFile("canary-latest.txt", currentGitSha);
    await $`gsutil -h "Cache-Control: no-cache" cp canary-latest.txt gs://dl.deno.land/canary-latest.txt`;
  } else {
    console.log("Skipping upload, newer canary version is already available");
  }
}

function printUsage() {
  throw new Error(
    "Usage: `./upload_canary_hash.ts arch|latest <currentGitSha>",
  );
}

async function main() {
  const kind = Deno.args[0];
  const currentGitSha = Deno.args[1];
  if (!kind || !currentGitSha) {
    printUsage();
  }
  if (kind === "arch") {
    await uploadCanaryForCurrentArch(currentGitSha);
  } else if (kind === "latest") {
    await uploadCanaryLatest(currentGitSha);
  } else {
    printUsage();
  }
}

await main();
