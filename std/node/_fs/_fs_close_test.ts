// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { test } = Deno;
import { fail, assert } from "../../testing/asserts.ts";
import { close, closeSync } from "./_fs_close.ts";

test({
  name: "ASYNC: File is closed",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    const file: Deno.File = await Deno.open(tempFile);

    assert(Deno.resources()[file.rid]);
    await new Promise((resolve, reject) => {
      close(file.rid, (err) => {
        if (err) reject();
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

test({
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
