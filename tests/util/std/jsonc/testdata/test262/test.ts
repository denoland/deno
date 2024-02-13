// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { walk } from "../../../fs/mod.ts";
import { fromFileUrl } from "../../../path/mod.ts";

// helper used for testing
const sta = await Deno.readTextFile(new URL("./sta.js", import.meta.url));
const assert = await Deno.readTextFile(new URL("./assert.js", import.meta.url));
const propertyHelper = await Deno.readTextFile(
  new URL("./propertyHelper.js", import.meta.url),
);
const jsoncModule = new URL("../../parse.ts", import.meta.url);
for await (
  const dirEntry of walk(fromFileUrl(new URL("./JSON/", import.meta.url)))
) {
  if (!dirEntry.isFile) {
    continue;
  }
  // Register a test case for each file.
  Deno.test({
    name: `[jsonc] parse test262:${dirEntry.name}`,
    async fn() {
      // Run the test case to make sure there are no errors.
      // Check if the JSONC module passes the test case for JSON.parse.
      const testcode = `
        import * as JSONC from "${jsoncModule}";
        const JSON = JSONC;
        ${sta}
        ${assert}
        ${propertyHelper}
        ${await Deno.readTextFile(dirEntry.path)}
      `;
      await import(`data:text/javascript,${encodeURIComponent(testcode)}`);
    },
  });
}
