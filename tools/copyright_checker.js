#!/usr/bin/env -S deno run --unstable --allow-read --allow-run
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import {
  getSources,
  ROOT_PATH,
} from "./util.js";

async function checkCopyright() {
  const sourceFiles = await getSources(ROOT_PATH, [
    // js and ts
    "*.js",
    "*.ts",
    ":!:.github/mtime_cache/action.js",
    ":!:cli/tests/testdata/**",
    ":!:cli/bench/testdata/**",
    ":!:cli/tsc/dts/**",
    ":!:cli/tsc/*typescript.js",
    ":!:cli/tsc/compiler.d.ts",
    ":!:test_util/wpt/**",
    ":!:tools/**", // these files are starts with `#!/usr/bin/env`
    ":!:cli/tools/init/templates/**",
    
    // rust
    "*.rs",
    ":!:ops/optimizer_tests/**",

    // toml
    "*Cargo.toml"
  ]);

  if (!sourceFiles.length) {
    return;
  }

  let totalCount = 0;
  for (const file of sourceFiles) {
    const fileText = await Deno.readTextFile(file);
    if (file.endsWith("Cargo.toml")) {
      if (!fileText.startsWith("# Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.")) {
        console.log("Copyright is not sign: " + file);
        totalCount += 1;
      }
      continue;
    }

    if (!fileText.startsWith("// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.")) {
      console.log("Copyright is not sign: " + file);
      totalCount += 1;
    }
  }

  console.log("\nTotal count: " + totalCount);

  if (totalCount > 0) {
    Deno.exit(1);
  }
}

async function main() {
  await Deno.chdir(ROOT_PATH);

  await checkCopyright();
}

await main();