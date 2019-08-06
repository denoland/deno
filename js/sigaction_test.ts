// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assertEquals, deferred } from "./test_util.ts";

// Ignore signal tests on windows for now...
if (Deno.platform.os !== "win") {
  testPerm({ run: true, net: true }, async function sigactionTest(): Promise<
    void
  > {
    const d = deferred();
    let signalCaughtCount = 0;
    Deno.sigaction(
      Deno.Signal.SIGUSR1,
      (): void => {
        signalCaughtCount++;
      }
    );

    Deno.sigaction(
      Deno.Signal.SIGUSR1,
      (): void => {
        signalCaughtCount++;
        d.resolve();
      }
    );

    // Since signal listening is optional, Deno would exit
    // immediately once there is no required tasks.
    // To prevent this during testing, listen on a port
    // for no reason such that we have required task
    // to prevent exit.
    const l = Deno.listen("tcp", "localhost:4999");

    Deno.kill(Deno.pid, Deno.Signal.SIGUSR1);
    await d;
    assertEquals(signalCaughtCount, 2);

    l.close();
  });
}
