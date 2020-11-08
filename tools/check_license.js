#!/usr/bin/env -S deno run --unstable --allow-read
// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { checkLicense } from "https://deno.land/x/license_checker@v3.1.0/lib.ts";
import { ROOT_PATH } from "./util.js";

await Deno.chdir(ROOT_PATH);
console.log("checking license headers");

const success = await checkLicense([{
  config: [
    [
      "**/*.{ts,js,toml,rs}",
      "Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.",
    ],
  ],
  ignore: [
    "testdata",
    "cli/dts",
    "cli/tests",
    "std/node/tests",
    "std/hash/_wasm/wasm.js",
    "cli/tsc/00_typescript.js",
    "target",
  ],
}], { quiet: true });

if (!success) {
  console.log("license check failed");
  Deno.exit(1);
}
console.log("ok");
