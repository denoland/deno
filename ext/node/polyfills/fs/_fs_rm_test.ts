// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertRejects, fail } from "../../testing/asserts.ts";
import { rm } from "./promises.ts";
import { closeSync, existsSync } from "../fs.ts";
import { join } from "../../path/mod.ts";
import { isWindows } from "../../_util/os.ts";

Deno.test({
  name: "ASYNC: removing empty folder",
  async fn() {
    const dir = Deno.makeTempDirSync();
    await rm(dir, { recursive: true })
      .then(() => assertEquals(existsSync(dir), false), () => fail())
      .finally(() => {
        if (existsSync(dir)) Deno.removeSync(dir);
      });
  },
});

function closeRes(before: Deno.ResourceMap, after: Deno.ResourceMap) {
  for (const key in after) {
    if (!before[key]) {
      try {
        closeSync(Number(key));
      } catch (error) {
        return error;
      }
    }
  }
}

Deno.test({
  name: "removing non-empty folder",
  async fn() {
    const rBefore = Deno.resources();
    const dir = Deno.makeTempDirSync();
    Deno.createSync(join(dir, "file1.txt"));
    Deno.createSync(join(dir, "file2.txt"));
    Deno.mkdirSync(join(dir, "some_dir"));
    Deno.createSync(join(dir, "some_dir", "file.txt"));
    await rm(dir, { recursive: true })
      .then(() => assertEquals(existsSync(dir), false), () => fail())
      .finally(() => {
        if (existsSync(dir)) Deno.removeSync(dir, { recursive: true });
        const rAfter = Deno.resources();
        closeRes(rBefore, rAfter);
      });
  },
  ignore: isWindows,
});

Deno.test({
  name: "removing a file",
  async fn() {
    const file = Deno.makeTempFileSync();
    await rm(file);
    assertEquals(existsSync(file), false);
  },
});

Deno.test({
  name: "remove should fail if target does not exist",
  async fn() {
    await assertRejects(() => rm("/path/to/noexist.text"), Error);
  },
});

Deno.test({
  name:
    "remove should not fail if target does not exist and force option is true",
  async fn() {
    await rm("/path/to/noexist.text", { force: true });
  },
});
