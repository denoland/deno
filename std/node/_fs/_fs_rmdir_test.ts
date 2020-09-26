import { assertEquals, fail } from "../../testing/asserts.ts";
import { rmdir, rmdirSync } from "./_fs_rmdir.ts";
import { existsSync } from "../../fs/mod.ts";
import { join } from "../../path/mod.ts";

Deno.test({
  name: "ASYNC: removing empty folder",
  async fn() {
    const dir = Deno.makeTempDirSync();
    await new Promise((resolve, reject) => {
      rmdir(dir, (err) => {
        if (err) reject(err);
        resolve();
      });
    })
      .then(() => assertEquals(existsSync(dir), false))
      .catch(() => fail())
      .finally(() => {
        if (existsSync(dir)) Deno.removeSync(dir);
      });
  },
});

Deno.test({
  name: "SYNC: removing empty folder",
  fn() {
    const dir = Deno.makeTempDirSync();
    rmdirSync(dir);
    assertEquals(existsSync(dir), false);
  },
});

Deno.test({
  name: "ASYNC: removing non-empty folder",
  async fn() {
    const dir = Deno.makeTempDirSync();
    Deno.createSync(join(dir, "file1.txt"));
    Deno.createSync(join(dir, "file2.txt"));
    Deno.mkdirSync(join(dir, "some_dir"));
    Deno.createSync(join(dir, "some_dir", "file.txt"));
    await new Promise((resolve, reject) => {
      rmdir(dir, { recursive: true }, (err) => {
        if (err) reject(err);
        resolve();
      });
    })
      .then(() => assertEquals(existsSync(dir), false))
      .catch(() => fail())
      .finally(() => {
        if (existsSync(dir)) Deno.removeSync(dir, { recursive: true });
      });
  },
});

Deno.test({
  name: "SYNC: removing non-empty folder",
  fn() {
    const dir = Deno.makeTempDirSync();
    Deno.createSync(join(dir, "file1.txt"));
    Deno.createSync(join(dir, "file2.txt"));
    Deno.mkdirSync(join(dir, "some_dir"));
    Deno.createSync(join(dir, "some_dir", "file.txt"));
    rmdirSync(dir, { recursive: true });
    assertEquals(existsSync(dir), false);
  },
});
