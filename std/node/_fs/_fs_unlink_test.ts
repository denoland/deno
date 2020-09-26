import { assertEquals, assertThrows } from "../../testing/asserts.ts";
import { existsSync } from "../../fs/mod.ts";
import { unlink, unlinkSync } from "./_fs_unlink.ts";

Deno.test({
  name: "No callback Fn results in Error",
  fn() {
    assertThrows(
      () => {
        // @ts-ignore
        unlink(Deno.makeTempFileSync());
      },
      Error,
      "No callback function supplied",
    );
  },
});

Deno.test({
  name: "Test unlink",
  fn() {
    const file = Deno.makeTempFileSync();
    unlink(file, (err) => {
      if (err) throw err;
      assertEquals(existsSync(file), false);
    });
  },
});

Deno.test({
  name: "Test unlink (sync)",
  fn() {
    const file = Deno.makeTempFileSync();
    unlinkSync(file);
    assertEquals(existsSync(file), false);
  },
});
