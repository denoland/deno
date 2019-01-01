#!/usr/bin/env deno --allow-run
// This program fails if ./tools/format.ts changes any files.

import { cwd, exit, RunOptions } from "deno";
import {
  findFiles,
  ProcessOptions,
  resolveProcess,
  ProcessResult
} from "./third_party.ts";

async function main() {
  // Format code
  await import("./format.ts");

  // Check for git changes
  const { message, success, stdout } = await resolveProcess([
    "git status",
    {
      args: ["git", "status", "-uno", "--porcelain", "--ignore-submodules"]
    }
  ]);

  if (await stdout) {
    console.log(message);
    console.log("✖ files changed");
    console.log(await stdout);
    exit(1);
  }

  if (message) {
    return Promise.reject(message);
  }

  if (!success) {
    return Promise.reject("git process failed");
  }
}

main();
