// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertCallbackErrorUncaught } from "../_test_utils.ts";
import { readlink, readlinkSync } from "./_fs_readlink.ts";
import { assert, assertEquals } from "../../testing/asserts.ts";
import * as path from "../path.ts";
import { isWindows } from "../../_util/os.ts";

const testDir = Deno.makeTempDirSync();
const oldname = path.join(testDir, "oldname");
const newname = path.join(testDir, "newname");

if (isWindows) {
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
  const importUrl = new URL("./_fs_readlink.ts", import.meta.url);
  await assertCallbackErrorUncaught({
    prelude: `import { readlink } from ${JSON.stringify(importUrl)}`,
    invocation: `readlink(${JSON.stringify(newname)}, `,
  });
});
