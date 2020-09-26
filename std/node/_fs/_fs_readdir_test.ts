import { assertEquals, assertThrows } from "../../testing/asserts.ts";
import { readdir, readdirSync } from "./_fs_readdir.ts";
import { join } from "../../path/mod.ts";

Deno.test({
  name: "No callback Fn results in Error",
  fn() {
    assertThrows(
      () => {
        // @ts-ignore
        readdir(Deno.makeTempDirSync());
      },
      Error,
      "No callback function supplied",
    );
  },
});

Deno.test({
  name: "Test reading empty the directory",
  fn() {
    const dir = Deno.makeTempDirSync();
    readdir(dir, (err, files) => {
      if (err) throw err;
      assertEquals(files, []);
    });
  },
});

Deno.test({
  name: "Test reading a directory that's not empty",
  fn() {
    const dir = Deno.makeTempDirSync();
    Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
    Deno.writeTextFileSync(join(dir, "file2.txt"), "hi");
    Deno.mkdirSync(join(dir, "some_dir"));
    readdir(dir, (err, files) => {
      if (err) throw err;
      assertEquals(files, ["file1.txt", "file2.txt", "some_dir"]);
    });
  },
});

Deno.test({
  name: "Test reading empty the directory (sync)",
  fn() {
    const dir = Deno.makeTempDirSync();
    assertEquals(readdirSync(dir), []);
  },
});

Deno.test({
  name: "Test reading a directory that's not empty (sync)",
  fn() {
    const dir = Deno.makeTempDirSync();
    Deno.writeTextFileSync(join(dir, "file1.txt"), "hi");
    Deno.writeTextFileSync(join(dir, "file2.txt"), "hi");
    Deno.mkdirSync(join(dir, "some_dir"));
    assertEquals(readdirSync(dir), ["file1.txt", "file2.txt", "some_dir"]);
  },
});
