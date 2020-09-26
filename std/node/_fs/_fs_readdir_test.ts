import { assertEquals, fail } from "../../testing/asserts.ts";
import { readdir, readdirSync } from "./_fs_readdir.ts";
import { join } from "../../path/mod.ts";

Deno.test({
  name: "ASYNC: reading empty directory",
  async fn() {
    const dir = Deno.makeTempDirSync();
    await new Promise<string[]>((resolve, reject) => {
      readdir(dir, (err, files) => {
        if (err) reject(err);
        resolve(files);
      });
    })
      .then((files) => assertEquals(files, []))
      .catch(() => fail())
      .finally(() => Deno.removeSync(dir));
  },
});

Deno.test({
  name: "ASYNC: reading non-empty directory",
  async fn() {
    const dir = Deno.makeTempDirSync();
    Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
    Deno.writeTextFileSync(join(dir, "file2.txt"), "hi");
    Deno.mkdirSync(join(dir, "some_dir"));
    await new Promise((resolve, reject) => {
      readdir(dir, (err, files) => {
        if (err) reject(err);
        resolve(files);
      });
    })
      .then((files) =>
        assertEquals(files, ["file1.txt", "some_dir", "file2.txt"])
      )
      .catch(() => fail())
      .finally(() => Deno.removeSync(dir, { recursive: true }));
  },
});

Deno.test({
  name: "SYNC: reading empty the directory",
  fn() {
    const dir = Deno.makeTempDirSync();
    assertEquals(readdirSync(dir), []);
  },
});

Deno.test({
  name: "SYNC: reading a non-empty directory",
  fn() {
    const dir = Deno.makeTempDirSync();
    Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
    Deno.writeTextFileSync(join(dir, "file2.txt"), "hi");
    Deno.mkdirSync(join(dir, "some_dir"));
    assertEquals(readdirSync(dir), ["file1.txt", "some_dir", "file2.txt"]);
  },
});
