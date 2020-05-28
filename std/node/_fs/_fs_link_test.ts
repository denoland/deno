// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { test } = Deno;
import { fail, assertEquals } from "../../testing/asserts.ts";
import { link, linkSync } from "./_fs_link.ts";
import { writeFileStrSync } from "../../fs/write_file_str.ts";
import { readFileStrSync } from "../../fs/read_file_str.ts";
import { assert } from "https://deno.land/std@v0.50.0/testing/asserts.ts";
const isWindows = Deno.build.os === "windows";

test({
  ignore: isWindows,
  name: "ASYNC: hard linking files works as expected",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    await new Promise((res, rej) => {
      link(tempFile, tempFile + ".link", (err) => {
        if (err) rej(err);
        else res();
      });
    })
      .then(() => {
        writeFileStrSync(tempFile, "hello world");
        assertEquals(readFileStrSync(tempFile + ".link"), "hello world");
      })
      .catch(() => {
        fail("Expected to succeed");
      })
      .finally(() => {
        Deno.removeSync(tempFile);
        Deno.removeSync(tempFile + ".link");
      });
  },
});

test({
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

test({
  ignore: isWindows,
  name: "SYNC: hard linking files works as expected",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    linkSync(tempFile, tempFile + ".link");

    writeFileStrSync(tempFile, "hello world");
    assertEquals(readFileStrSync(tempFile + ".link"), "hello world");
    Deno.removeSync(tempFile);
    Deno.removeSync(tempFile + ".link");
  },
});
