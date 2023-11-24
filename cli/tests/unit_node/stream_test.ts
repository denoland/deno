// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assert, fail } from "../../../test_util/std/assert/mod.ts";
import { fromFileUrl, relative } from "../../../test_util/std/path/mod.ts";
import { pipeline } from "node:stream/promises";
import { Writable } from "node:stream";
import { createReadStream, createWriteStream } from "node:fs";

Deno.test("stream/promises pipeline", async () => {
  const filePath = relative(
    Deno.cwd(),
    fromFileUrl(new URL("./testdata/lorem_ipsum.txt", import.meta.url)),
  );
  const input = createReadStream(filePath);
  const output = createWriteStream("lorem_ipsum.txt.copy");

  await pipeline(input, output);

  const content = Deno.readTextFileSync("lorem_ipsum.txt.copy");
  assert(content.startsWith("Lorem ipsum dolor sit amet"));
  try {
    Deno.removeSync("lorem_ipsum.txt.copy");
  } catch {
    // pass
  }
});

// TODO(kt3k): Remove this test case when the node compat test suite is
// updated to version 18.16.0 or above.
// The last case in parallel/test-stream2-transform.js covers this case.
// See https://github.com/nodejs/node/pull/46818
Deno.test("stream.Writable does not change the order of items", async () => {
  async function test() {
    const chunks: Uint8Array[] = [];
    const writable = new Writable({
      construct(cb) {
        setTimeout(cb, 10);
      },
      write(chunk, _, cb) {
        chunks.push(chunk);
        cb();
      },
    });

    for (const i of Array(20).keys()) {
      writable.write(Uint8Array.from([i]));
      await new Promise((resolve) => setTimeout(resolve, 1));
    }

    if (chunks[0][0] !== 0) {
      // The first chunk is swapped with the later chunk.
      fail("The first chunk is swapped");
    }
  }

  for (const _ of Array(10)) {
    // Run it multiple times to avoid flaky false negative.
    await test();
  }
});
