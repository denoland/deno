// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import * as path from "../../path/mod.ts";
import { assert, assertEquals, fail } from "../../testing/asserts.ts";
import { assertCallbackErrorUncaught } from "../_utils.ts";
import { link, linkSync } from "./_fs_link.ts";

Deno.test({
  name: "ASYNC: hard linking files works as expected",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    const linkedFile: string = tempFile + ".link";
    await new Promise<void>((res, rej) => {
      link(tempFile, linkedFile, (err) => {
        if (err) rej(err);
        else res();
      });
    })
      .then(() => {
        assertEquals(Deno.statSync(tempFile), Deno.statSync(linkedFile));
      }, () => {
        fail("Expected to succeed");
      })
      .finally(() => {
        Deno.removeSync(tempFile);
        Deno.removeSync(linkedFile);
      });
  },
});

Deno.test({
  name: "ASYNC: hard linking files passes error to callback",
  async fn() {
    let failed = false;
    await new Promise<void>((res, rej) => {
      link("no-such-file", "no-such-file", (err) => {
        if (err) rej(err);
        else res();
      });
    })
      .then(() => {
        fail("Expected to succeed");
      }, (err) => {
        assert(err);
        failed = true;
      });
    assert(failed);
  },
});

Deno.test({
  name: "SYNC: hard linking files works as expected",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const linkedFile: string = tempFile + ".link";
    linkSync(tempFile, linkedFile);

    assertEquals(Deno.statSync(tempFile), Deno.statSync(linkedFile));
    Deno.removeSync(tempFile);
    Deno.removeSync(linkedFile);
  },
});

Deno.test("[std/node/fs] link callback isn't called twice if error is thrown", async () => {
  const tempDir = await Deno.makeTempDir();
  const tempFile = path.join(tempDir, "file.txt");
  const linkFile = path.join(tempDir, "link.txt");
  await Deno.writeTextFile(tempFile, "hello world");
  const importUrl = new URL("./_fs_link.ts", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { link } from ${JSON.stringify(importUrl)}`,
    invocation: `link(${JSON.stringify(tempFile)}, 
                      ${JSON.stringify(linkFile)}, `,
    async cleanup() {
      await Deno.remove(tempDir, { recursive: true });
    },
  });
});
