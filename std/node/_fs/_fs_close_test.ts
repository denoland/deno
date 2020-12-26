// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertStringIncludes,
  assertThrows,
  fail,
} from "../../testing/asserts.ts";
import { close, closeSync } from "./_fs_close.ts";

Deno.test({
  name: "ASYNC: File is closed",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    const file: Deno.File = await Deno.open(tempFile);

    assert(Deno.resources()[file.rid]);
    await new Promise<void>((resolve, reject) => {
      close(file.rid, (err) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(() => {
        assert(!Deno.resources()[file.rid]);
      }, () => {
        fail("No error expected");
      })
      .finally(async () => {
        await Deno.remove(tempFile);
      });
  },
});

Deno.test({
  name: "ASYNC: Invalid fd",
  async fn() {
    await new Promise<void>((resolve, reject) => {
      close(-1, (err) => {
        if (err !== null) return resolve();
        reject();
      });
    });
  },
});

Deno.test({
  name: "close callback should be asynchronous",
  async fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const file: Deno.File = Deno.openSync(tempFile);

    let foo: string;
    const promise = new Promise<void>((resolve) => {
      close(file.rid, () => {
        assert(foo === "bar");
        resolve();
      });
      foo = "bar";
    });

    await promise;
    Deno.removeSync(tempFile);
  },
});

Deno.test({
  name: "SYNC: File is closed",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const file: Deno.File = Deno.openSync(tempFile);

    assert(Deno.resources()[file.rid]);
    closeSync(file.rid);
    assert(!Deno.resources()[file.rid]);
    Deno.removeSync(tempFile);
  },
});

Deno.test({
  name: "SYNC: Invalid fd",
  fn() {
    assertThrows(() => closeSync(-1));
  },
});

Deno.test("[std/node/fs] close callback isn't called twice if error is thrown", async () => {
  // The correct behaviour is not to catch any errors thrown,
  // but that means there'll be an uncaught error and the test will fail.
  // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
  // (assertThrowsAsync won't work because there's no way to catch the error.)
  const tempFile = await Deno.makeTempFile();
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "--no-check",
      `
      import { close } from "${
        new URL("./_fs_close.ts", import.meta.url).href
      }";

      const file = await Deno.open(${JSON.stringify(tempFile)});

      close(file.rid, (err) => {
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
  await Deno.remove(tempFile);
  assert(!status.success);
  assertStringIncludes(stderr, "Error: success");
});
