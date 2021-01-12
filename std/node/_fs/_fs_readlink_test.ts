// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { readlink, readlinkSync } from "./_fs_readlink.ts";
import {
  assert,
  assertEquals,
  assertStringIncludes,
} from "../../testing/asserts.ts";
import * as path from "../path.ts";

const testDir = Deno.makeTempDirSync();
const oldname = path.join(testDir, "oldname");
const newname = path.join(testDir, "newname");

if (Deno.build.os === "windows") {
  Deno.symlinkSync(oldname, newname, { type: "file" });
} else {
  Deno.symlinkSync(oldname, newname);
}

Deno.test({
  name: "readlinkSuccess",
  async fn() {
    const data = await new Promise((res, rej) => {
      readlink(newname, (err, data) => {
        if (err) {
          rej(err);
        }
        res(data);
      });
    });

    assertEquals(typeof data, "string");
    assertEquals(data as string, oldname);
  },
});

Deno.test({
  name: "readlinkEncodeBufferSuccess",
  async fn() {
    const data = await new Promise((res, rej) => {
      readlink(newname, { encoding: "buffer" }, (err, data) => {
        if (err) {
          rej(err);
        }
        res(data);
      });
    });

    assert(data instanceof Uint8Array);
    assertEquals(new TextDecoder().decode(data as Uint8Array), oldname);
  },
});

Deno.test({
  name: "readlinkSyncSuccess",
  fn() {
    const data = readlinkSync(newname);
    assertEquals(typeof data, "string");
    assertEquals(data as string, oldname);
  },
});

Deno.test({
  name: "readlinkEncodeBufferSuccess",
  fn() {
    const data = readlinkSync(newname, { encoding: "buffer" });
    assert(data instanceof Uint8Array);
    assertEquals(new TextDecoder().decode(data as Uint8Array), oldname);
  },
});

Deno.test("[std/node/fs] readlink callback isn't called twice if error is thrown", async () => {
  // The correct behaviour is not to catch any errors thrown,
  // but that means there'll be an uncaught error and the test will fail.
  // So the only way to test this is to spawn a subprocess, and succeed if it has a non-zero exit code.
  // (assertThrowsAsync won't work because there's no way to catch the error.)
  const p = Deno.run({
    cmd: [
      Deno.execPath(),
      "eval",
      "--no-check",
      `
      import { readlink } from "${
        new URL("./_fs_readlink.ts", import.meta.url).href
      }";

      readlink(${JSON.stringify(newname)}, (err) => {
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
  assert(!status.success);
  assertStringIncludes(stderr, "Error: success");
});
