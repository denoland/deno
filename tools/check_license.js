#!/usr/bin/env -S deno run --unstable --allow-read --allow-run
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { ROOT_PATH } from "./util.js";

await Deno.chdir(ROOT_PATH);
console.log("checking license headers");

const p = Deno.run(
  {
    cmd: [
      "deno",
      "run",
      "--unstable",
      "--allow-read",
      "https://deno.land/x/license_checker@v3.0.4/main.ts",
      "-q",
    ],
  },
);
const { success } = await p.status();
if (!success) {
  throw new Error("license check failed");
}
console.log("ok");
p.close();
