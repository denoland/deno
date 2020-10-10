import { watch } from "./_fs_watch.ts";
import { assertEquals, fail } from "../../testing/asserts.ts";

function wait(time: number) {
  return new Promise((resolve) => {
    setTimeout(resolve, time);
  });
}

Deno.test({
  name: "watching a file",
  async fn() {
    const file = Deno.makeTempFileSync();
    const result: Array<[string, string]> = [];
    await new Promise((resolve) => {
      const watcher = watch(
        file,
        (eventType, filename) => result.push([eventType, filename]),
      );
      wait(100)
        .then(() => Deno.writeTextFileSync(file, "something"))
        .then(() => wait(100))
        .then(() => watcher.close())
        .then(() => wait(100))
        .then(resolve);
    })
      .then(() => {
        assertEquals(result.length >= 1, true);
      })
      .catch(() => fail());
  },
});
