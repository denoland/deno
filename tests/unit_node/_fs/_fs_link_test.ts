// Copyright 2018-2025 the Deno authors. MIT license.
import * as path from "@std/path";
import { assert, assertEquals, fail } from "@std/assert";
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { link, linkSync } from "node:fs";
import { Buffer } from "node:buffer";

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
  const importUrl = new URL("node:fs", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { link } from ${JSON.stringify(importUrl)}`,
    invocation: `link(${JSON.stringify(tempFile)}, 
                      ${JSON.stringify(linkFile)}, `,
    async cleanup() {
      await Deno.remove(tempDir, { recursive: true });
    },
  });
});

Deno.test("[std/node/fs] link accepts Buffer", async () => {
  const tempDir = await Deno.makeTempDir();
  const tempFile = path.join(tempDir, "file.txt");
  const linkedFile = path.join(tempDir, "file.link");
  const tempFileBuffer = Buffer.from(tempFile, "utf8");
  const linkedFileBuffer = Buffer.from(linkedFile, "utf8");
  await Deno.writeTextFile(tempFile, "hello world");

  await new Promise<void>((resolve, reject) => {
    link(tempFileBuffer, linkedFileBuffer, (err) => {
      if (err) reject(err);
      else resolve();
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
      Deno.removeSync(tempDir);
    });
});

Deno.test("[std/node/fs] linkSync accepts Buffer", () => {
  const tempDir = Deno.makeTempDirSync();
  const tempFile = path.join(tempDir, "file.txt");
  const linkedFile = path.join(tempDir, "file.link");
  const tempFileBuffer = Buffer.from(tempFile, "utf8");
  const linkedFileBuffer = Buffer.from(linkedFile, "utf8");
  Deno.writeTextFileSync(tempFile, "hello world");

  linkSync(tempFileBuffer, linkedFileBuffer);
  assertEquals(Deno.statSync(tempFile), Deno.statSync(linkedFile));

  Deno.removeSync(linkedFile);
  Deno.removeSync(tempFile);
  Deno.removeSync(tempDir);
});
