// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { fail, assert, assertThrows } from "../../testing/asserts.ts";
import { close, closeSync } from "./_fs_close.ts";

Deno.test({
  name: "ASYNC: File is closed",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    const file: Deno.File = await Deno.open(tempFile);

    assert(Deno.resources()[file.rid]);
    await new Promise((resolve, reject) => {
      close(file.rid, (err) => {
        if (err !== null) reject();
        else resolve();
      });
    })
      .then(() => {
        assert(!Deno.resources()[file.rid]);
      })
      .catch(() => {
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
    await new Promise((resolve, reject) => {
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
    const promise = new Promise((resolve) => {
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
