import { assertEquals, assertThrows } from "../../testing/asserts.ts";
import { rename, renameSync } from "./_fs_rename.ts";
import { existsSync } from "../../fs/mod.ts";
import { join, parse } from "../../path/mod.ts";

Deno.test({
  name: "No callback Fn results in Error",
  fn() {
    assertThrows(
      () => {
        // @ts-ignore
        rename(Deno.makeTempDirSync(), "some_thing");
      },
      Error,
      "No callback function supplied",
    );
  },
});

Deno.test({
  name: "Test renaming",
  fn() {
    const file = Deno.makeTempFileSync();
    const newPath = join(parse(file).dir, `${parse(file).base}_renamed`);
    rename(file, newPath, (err) => {
      if (err) throw err;
      assertEquals(existsSync(newPath), true);
      assertEquals(existsSync(file), false);
    });
  },
});

Deno.test({
  name: "Test renaming (sync)",
  fn() {
    const file = Deno.makeTempFileSync();
    const newPath = join(parse(file).dir, `${parse(file).base}_renamed`);
    renameSync(file, newPath);
    assertEquals(existsSync(newPath), true);
    assertEquals(existsSync(file), false);
  },
});
