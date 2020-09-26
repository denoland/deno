import { assertEquals, assertThrows } from "../../testing/asserts.ts";
import { rmdir, rmdirSync } from "./_fs_rmdir.ts";
import { existsSync } from "../../fs/mod.ts";
import { join } from "../../path/mod.ts";

Deno.test({
  name: "No callback Fn results in Error",
  fn() {
    assertThrows(
      () => {
        // @ts-ignore
        rmdir(Deno.makeTempDirSync());
      },
      Error,
      "No callback function supplied",
    );
  },
});

Deno.test({
  name: "Test removing empty folder",
  fn() {
    const dir = Deno.makeTempDirSync();
    rmdir(dir, (err) => {
      if (err) throw err;
      assertEquals(existsSync(dir), false);
    });
  },
});

Deno.test({
  name: "Test removing empty folder (sync)",
  fn() {
    const dir = Deno.makeTempDirSync();
    rmdirSync(dir);
    assertEquals(existsSync(dir), false);
  },
});

Deno.test({
  name: "Test removing non-empty folder",
  fn() {
    const dir = Deno.makeTempDirSync();
    Deno.createSync(join(dir, "file1.txt"));
    Deno.createSync(join(dir, "file2.txt"));
    Deno.mkdirSync(join(dir, "some_dir"));
    Deno.createSync(join(dir, "some_dir", "file.txt"));
    rmdir(dir, { recursive: true }, (err) => {
      if (err) throw err;
      assertEquals(existsSync(dir), false);
    });
  },
});

Deno.test({
  name: "Test removing non-empty folder (sync)",
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
