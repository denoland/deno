import { join } from "../../path/mod.ts";
import { assertEquals, assertNotEquals, fail } from "../../testing/asserts.ts";
import { readdir, readdirSync } from "./_fs_readdir.ts";

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

function assertEqualsArrayAnyOrder<T>(actual: T[], expected: T[]) {
  assertEquals(actual.length, expected.length);
  for (const item of expected) {
    const index = actual.indexOf(item);
    assertNotEquals(index, -1);
    expected = expected.splice(index, 1);
  }
}

Deno.test({
  name: "ASYNC: reading non-empty directory",
  async fn() {
    const dir = Deno.makeTempDirSync();
    Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
    Deno.writeTextFileSync(join(dir, "file2.txt"), "hi");
    Deno.mkdirSync(join(dir, "some_dir"));
    await new Promise<string[]>((resolve, reject) => {
      readdir(dir, (err, files) => {
        if (err) reject(err);
        resolve(files);
      });
    })
      .then((files) =>
        assertEqualsArrayAnyOrder(files, ["file1.txt", "some_dir", "file2.txt"])
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
  name: "SYNC: reading non-empty directory",
  fn() {
    const dir = Deno.makeTempDirSync();
    Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
    Deno.writeTextFileSync(join(dir, "file2.txt"), "hi");
    Deno.mkdirSync(join(dir, "some_dir"));
    assertEqualsArrayAnyOrder(
      readdirSync(dir),
      ["file1.txt", "some_dir", "file2.txt"],
    );
  },
});
