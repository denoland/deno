// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This example is designed to simply write out a "hello world!" to a temporary
// file, read it back in, and log its contents to the console.  If you wanted
// to run the example in a browser, you would want to bundle it:
//
//     $ deno bundle https://deno.land/x/std/browser/examples/main.ts example.js
//
// You would then want to load the example as a module in a web page, like:
//
//     <script type="module" src="./example.js"></script>
//
// And then navigate to that web page to see the results.
//

import { init } from "../deno_shim.ts";

async function main(): Promise<void> {
  await init();
  const tmpFile = await Deno.makeTempFile();
  await Deno.writeTextFile(tmpFile, "hello world!");
  const txt = await Deno.readTextFile(tmpFile);
  console.log(txt);
}

main();
