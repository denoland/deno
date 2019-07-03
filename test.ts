#!/usr/bin/env deno run -A
// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import "./archive/tar_test.ts";
import "./bytes/test.ts";
import "./bundle/test.ts";
import "./colors/test.ts";
import "./datetime/test.ts";
import "./encoding/test.ts";
import "./examples/test.ts";
import "./flags/test.ts";
import "./fs/test.ts";
import "./http/test.ts";
import "./io/test.ts";
import "./installer/test.ts";
import "./log/test.ts";
import "./media_types/test.ts";
import "./mime/test.ts";
import "./multipart/test.ts";
import "./prettier/test.ts";
import "./strings/test.ts";
import "./testing/test.ts";
import "./textproto/test.ts";
import "./util/test.ts";
import "./uuid/test.ts";
import "./ws/test.ts";
import "./encoding/test.ts";
import "./os/test.ts";

import { xrun } from "./prettier/util.ts";
import { red, green } from "./colors/mod.ts";
import { runTests } from "./testing/mod.ts";

async function run(): Promise<void> {
  const startTime = Date.now();
  await runTests();
  await checkSourceFileChanges(startTime);
}

/**
 * Checks whether any source file is changed since the given start time.
 * If some files are changed, this function exits with 1.
 */
async function checkSourceFileChanges(startTime: number): Promise<void> {
  console.log("test checkSourceFileChanges ...");
  const changed = new TextDecoder()
    .decode(await xrun({ args: ["git", "ls-files"], stdout: "piped" }).output())
    .trim()
    .split("\n")
    .filter(file => {
      const stat = Deno.lstatSync(file);
      if (stat != null) {
        return (stat as any).modified * 1000 > startTime;
      }
    });
  if (changed.length > 0) {
    console.log(red("FAILED"));
    console.log(
      `Error: Some source files are modified during test: ${changed.join(", ")}`
    );
    Deno.exit(1);
  } else {
    console.log(green("ok"));
  }
}

run();
