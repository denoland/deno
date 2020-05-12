import { deferred } from "./deferred.ts";
import { delay } from "./delay.ts";
import { assertThrowsAsync, assertEquals } from "../testing/asserts.ts";
import { letTimeout } from "./timeout.ts";
import { TimeoutError } from "./timeout.ts";

const {test} = Deno;

test({
  name: "letTimeout",
  async fn() {
    const d = deferred();
    const wait = delay(20);
    wait.then(d.resolve).catch(d.resolve);
    await assertThrowsAsync(() => {
      return letTimeout(delay(20), 10);
    }, TimeoutError);
    await d;
  },
});

test({
  name: "letTimouet without timeout",
  async fn() {
    const p = Promise.resolve(true);
    const v = await letTimeout(p);
    assertEquals(v, true);
  },
});
