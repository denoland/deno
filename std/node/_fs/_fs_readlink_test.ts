const { test } = Deno;
import { readlink, readlinkSync } from "./_fs_readlink.ts";
import { assertEquals, assert } from "../../testing/asserts.ts";

const testDir = Deno.makeTempDirSync();
const oldname = testDir + "/oldname";
const newname = testDir + "/newname";

if (Deno.build.os !== "win") {
  Deno.symlinkSync(oldname, newname);
}

test({
  name: "readlinkSuccess",
  ignore: Deno.build.os === "win",
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

test({
  name: "readlinkEncodeBufferSuccess",
  ignore: Deno.build.os === "win",
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

test({
  name: "readlinkSyncSuccess",
  ignore: Deno.build.os === "win",
  fn() {
    const data = readlinkSync(newname);
    assertEquals(typeof data, "string");
    assertEquals(data as string, oldname);
  },
});

test({
  name: "readlinkEncodeBufferSuccess",
  ignore: Deno.build.os === "win",
  fn() {
    const data = readlinkSync(newname, { encoding: "buffer" });
    assert(data instanceof Uint8Array);
    assertEquals(new TextDecoder().decode(data as Uint8Array), oldname);
  },
});
