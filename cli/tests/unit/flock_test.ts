// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test(
  { permissions: { read: true, run: true, hrtime: true } },
  async function flockFileSync() {
    await runFlockTests({ sync: true });
  },
);

Deno.test(
  { permissions: { read: true, run: true, hrtime: true } },
  async function flockFileAsync() {
    await runFlockTests({ sync: false });
  },
);

async function runFlockTests(opts: { sync: boolean }) {
  assertEquals(
    await checkFirstBlocksSecond({
      firstExclusive: true,
      secondExclusive: false,
      sync: opts.sync,
    }),
    true,
    "exclusive blocks shared",
  );
  assertEquals(
    await checkFirstBlocksSecond({
      firstExclusive: false,
      secondExclusive: true,
      sync: opts.sync,
    }),
    true,
    "shared blocks exclusive",
  );
  assertEquals(
    await checkFirstBlocksSecond({
      firstExclusive: true,
      secondExclusive: true,
      sync: opts.sync,
    }),
    true,
    "exclusive blocks exclusive",
  );
  assertEquals(
    await checkFirstBlocksSecond({
      firstExclusive: false,
      secondExclusive: false,
      sync: opts.sync,
      // need to wait for both to enter the lock to prevent the case where the
      // first process enters and exits the lock before the second even enters
      waitBothEnteredLock: true,
    }),
    false,
    "shared does not block shared",
  );
}

async function checkFirstBlocksSecond(opts: {
  firstExclusive: boolean;
  secondExclusive: boolean;
  sync: boolean;
  waitBothEnteredLock?: boolean;
}) {
  const firstProcess = runFlockTestProcess({
    exclusive: opts.firstExclusive,
    sync: opts.sync,
  });
  const secondProcess = runFlockTestProcess({
    exclusive: opts.secondExclusive,
    sync: opts.sync,
  });
  try {
    const sleep = (time: number) => new Promise((r) => setTimeout(r, time));

    await Promise.all([
      firstProcess.waitStartup(),
      secondProcess.waitStartup(),
    ]);

    await firstProcess.enterLock();
    await firstProcess.waitEnterLock();

    await secondProcess.enterLock();
    await sleep(100);

    if (!opts.waitBothEnteredLock) {
      await firstProcess.exitLock();
    }

    await secondProcess.waitEnterLock();

    if (opts.waitBothEnteredLock) {
      await firstProcess.exitLock();
    }

    await secondProcess.exitLock();

    // collect the final output
    const firstPsTimes = await firstProcess.getTimes();
    const secondPsTimes = await secondProcess.getTimes();
    return firstPsTimes.exitTime < secondPsTimes.enterTime;
  } finally {
    await firstProcess.close();
    await secondProcess.close();
  }
}

function runFlockTestProcess(opts: { exclusive: boolean; sync: boolean }) {
  const path = "cli/tests/testdata/assets/fixture.json";
  const scriptText = `
    const { rid } = Deno.openSync("${path}");

    // ready signal
    Deno.stdout.writeSync(new Uint8Array(1));
    // wait for enter lock signal
    Deno.stdin.readSync(new Uint8Array(1));

    // entering signal
    Deno.stdout.writeSync(new Uint8Array(1));
    // lock and record the entry time
    ${
    opts.sync
      ? `Deno.flockSync(rid, ${opts.exclusive ? "true" : "false"});`
      : `await Deno.flock(rid, ${opts.exclusive ? "true" : "false"});`
  }
    const enterTime = new Date().getTime();
    // entered signal
    Deno.stdout.writeSync(new Uint8Array(1));

    // wait for exit lock signal
    Deno.stdin.readSync(new Uint8Array(1));

    // record the exit time and wait a little bit before releasing
    // the lock so that the enter time of the next process doesn't
    // occur at the same time as this exit time
    const exitTime = new Date().getTime();
    await new Promise(resolve => setTimeout(resolve, 100));

    // release the lock
    ${opts.sync ? "Deno.funlockSync(rid);" : "await Deno.funlock(rid);"}

    // exited signal
    Deno.stdout.writeSync(new Uint8Array(1));

    // output the enter and exit time
    console.log(JSON.stringify({ enterTime, exitTime }));
`;

  const process = new Deno.Command(Deno.execPath(), {
    args: ["eval", "--unstable", scriptText],
    stdin: "piped",
    stdout: "piped",
    stderr: "null",
  }).spawn();

  const waitSignal = async () => {
    const reader = process.stdout.getReader({ mode: "byob" });
    await reader.read(new Uint8Array(1));
    reader.releaseLock();
  };
  const signal = async () => {
    const writer = process.stdin.getWriter();
    await writer.write(new Uint8Array(1));
    writer.releaseLock();
  };

  return {
    async waitStartup() {
      await waitSignal();
    },
    async enterLock() {
      await signal();
      await waitSignal(); // entering signal
    },
    async waitEnterLock() {
      await waitSignal();
    },
    async exitLock() {
      await signal();
      await waitSignal();
    },
    getTimes: async () => {
      const { stdout } = await process.output();
      const text = new TextDecoder().decode(stdout);
      return JSON.parse(text) as {
        enterTime: number;
        exitTime: number;
      };
    },
    close: async () => {
      await process.status;
      await process.stdin.close();
    },
  };
}
