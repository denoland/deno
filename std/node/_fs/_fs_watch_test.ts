import { watch } from "./_fs_watch.ts";
import { assertEquals, fail } from "../../testing/asserts.ts";

Deno.test({
  name: "watching a file",
  fn() {
    const file = Deno.makeTempFileSync();
    const result: [string, string][] = [];
    const watcher = watch(file, (eventType, filename) =>
      result.push([eventType, filename])
    );
    Deno.writeTextFileSync(file, "something");
    watcher.close();
    assertEquals(result, [["modify", file]]);
  },
});
