import { assertEquals, fail } from "../../testing/asserts.ts";
import { rename, renameSync } from "./_fs_rename.ts";
import { existsSync } from "../../fs/mod.ts";
import { join, parse } from "../../path/mod.ts";

Deno.test({
  name: "ASYNC: renaming a file",
  async fn() {
    const file = Deno.makeTempFileSync();
    const newPath = join(parse(file).dir, `${parse(file).base}_renamed`);
    await new Promise((resolve, reject) => {
      rename(file, newPath, (err) => {
        if (err) reject(err);
      });
    })
      .then(() => {
        assertEquals(existsSync(newPath), true);
        assertEquals(existsSync(file), false);
      })
      .catch(() => fail())
      .finally(() => Deno.removeSync(file));
  },
});

Deno.test({
  name: "SYNC: renaming a file",
  fn() {
    const file = Deno.makeTempFileSync();
    const newPath = join(parse(file).dir, `${parse(file).base}_renamed`);
    renameSync(file, newPath);
    assertEquals(existsSync(newPath), true);
    assertEquals(existsSync(file), false);
  },
});
