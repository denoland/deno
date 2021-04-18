#!/usr/bin/env -S deno run --unstable --allow-read
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { checkLicense } from "https://deno.land/x/license_checker@v3.1.3/lib.ts";
import { ROOT_PATH } from "./util.js";

await Deno.chdir(ROOT_PATH);
console.log("checking license headers");

const success = await checkLicense([{
  config: [
    [
      "**/*.{ts,js,toml,rs}",
      "Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.",
    ],
  ],
  ignore: [
    "cli/dts",
    "cli/tests",
    "cli/bench/fixtures",
    "test_util/wpt",
    "test_util/std",
    "cli/tsc/00_typescript.js",
    "target",
  ],
}], { quiet: true });

if (!success) {
  console.log("license check failed");
  Deno.exit(1);
}
console.log("ok");
