const { test } = Deno;
import { assertEquals, assertThrows } from "../testing/asserts.ts";
import { delay } from "../util/async.ts";
import { signal } from "./mod.ts";

if (Deno.build.os !== "win") {
  test("signal() throws when called with empty signals", (): void => {
    assertThrows(
      () => {
        // @ts-ignore
        signal();
      },
      Error,
      "No signals are given. You need to specify at least one signal to create a signal stream."
    );
  });

  test({
    name: "signal() iterates for multiple signals",
    fn: async (): Promise<void> => {
      // This prevents the program from exiting.
      const t = setInterval(() => {}, 1000);

      let c = 0;
      const sig = signal(
        Deno.Signal.SIGUSR1,
        Deno.Signal.SIGUSR2,
        Deno.Signal.SIGINT
      );

      setTimeout(async () => {
        await delay(20);
        Deno.kill(Deno.pid, Deno.Signal.SIGINT);
        await delay(20);
        Deno.kill(Deno.pid, Deno.Signal.SIGUSR2);
        await delay(20);
        Deno.kill(Deno.pid, Deno.Signal.SIGUSR1);
        await delay(20);
        Deno.kill(Deno.pid, Deno.Signal.SIGUSR2);
        await delay(20);
        Deno.kill(Deno.pid, Deno.Signal.SIGUSR1);
        await delay(20);
        Deno.kill(Deno.pid, Deno.Signal.SIGINT);
        await delay(20);
        sig.dispose();
      });

      for await (const _ of sig) {
        c += 1;
      }

      assertEquals(c, 6);

      clearTimeout(t);
      // Clear timeout clears interval, but interval promise is not
      // yet resolved, delay to next turn of event loop otherwise,
      // we'll be leaking resources.
      await delay(10);
    },
  });
}
