// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertStringIncludes,
  fail,
} from "../../testing/asserts.ts";
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
  // The correct behaviour is not to catch any errors thrown,
  // but that means there'll be an uncaught error and the test will fail.
  // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
  // (assertThrowsAsync won't work because there's no way to catch the error.)
  const tempDir = await Deno.makeTempDir();
  await Deno.writeTextFile(`${tempDir}/file.txt`, "hello world");
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "--unstable",
      "--no-check",
      `
      import { link } from "${new URL("./_fs_link.ts", import.meta.url).href}";

      link(${JSON.stringify(`${tempDir}/file.txt`)}, ${
        JSON.stringify(tempDir)
      } + "/link.txt", (err) => {
        // If the bug is present and the callback is called again with an error,
        // don't throw another error, so if the subprocess fails we know it had the correct behaviour.
        if (!err) throw new Error("success");
      });`,
    ],
    stderr: "piped",
  });
  const status = await p.status();
  const stderr = new TextDecoder().decode(await Deno.readAll(p.stderr));
  p.close();
  p.stderr.close();
  await Deno.remove(tempDir, { recursive: true });
  assert(!status.success);
  assertStringIncludes(stderr, "Error: success");
});
