#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run=cargo --allow-net=crates.io --no-check --lock=tools/deno.lock.json
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { DenoWorkspace } from "./deno_workspace.ts";
import { Crate, getCratesPublishOrder } from "./deps.ts";

const isReal = parseIsReal();
console.log(`Running a ${isReal ? "real" : "dry"} cargo publish...`);

const workspace = await DenoWorkspace.load();
const cliCrate = workspace.getCliCrate();

const dependencyCrates = getCratesPublishOrder(
  workspace.getCliDependencyCrates(),
);

try {
  for (const [i, crate] of dependencyCrates.entries()) {
    await publishCrate(crate);
    console.log(`Finished ${i + 1} of ${dependencyCrates.length} crates.`);
  }

  await publishCrate(cliCrate);
} finally {
  // system beep to notify error or completion
  console.log("\x07");
}

async function publishCrate(crate: Crate) {
  if (isReal) {
    await crate.publish();
  } else {
    await crate.publishDryRun();
  }
}

function parseIsReal() {
  const isReal = Deno.args.some((a) => a === "--real");
  const isDry = Deno.args.some((a) => a === "--dry");

  // force the call to be explicit and provide one of these
  // so that it's obvious what's happening
  if (!isDry && !isReal) {
    console.error("Please run with `--dry` or `--real`.");
    Deno.exit(1);
  }

  return isReal;
}
