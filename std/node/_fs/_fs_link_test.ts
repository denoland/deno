// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { fail, assertEquals } from "../../testing/asserts.ts";
import { link, linkSync } from "./_fs_link.ts";
import { assert } from "https://deno.land/std@v0.50.0/testing/asserts.ts";

const isWindows = Deno.build.os === "windows";

Deno.test({
  ignore: isWindows,
  name: "ASYNC: hard linking files works as expected",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    const linkedFile: string = tempFile + ".link";
    await new Promise((res, rej) => {
      link(tempFile, linkedFile, (err) => {
        if (err) rej(err);
        else res();
      });
    })
      .then(() => {
        assertEquals(Deno.statSync(tempFile), Deno.statSync(linkedFile));
      })
      .catch(() => {
        fail("Expected to succeed");
      })
      .finally(() => {
        Deno.removeSync(tempFile);
        Deno.removeSync(linkedFile);
      });
  },
});

Deno.test({
  ignore: isWindows,
  name: "ASYNC: hard linking files passes error to callback",
  async fn() {
    let failed = false;
    await new Promise((res, rej) => {
      link("no-such-file", "no-such-file", (err) => {
        if (err) rej(err);
        else res();
      });
    })
      .then(() => {
        fail("Expected to succeed");
      })
      .catch((err) => {
        assert(err);
        failed = true;
      });
    assert(failed);
  },
});

Deno.test({
  ignore: isWindows,
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
