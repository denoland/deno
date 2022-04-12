// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";
import { readAll } from "../../../test_util/std/io/util.ts";

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
    firstProcess.close();
    secondProcess.close();
  }
}

function runFlockTestProcess(opts: { exclusive: boolean; sync: boolean }) {
  const path = "cli/tests/testdata/fixture.json";
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
    Deno.sleepSync(100);

    // release the lock
    ${opts.sync ? "Deno.funlockSync(rid);" : "await Deno.funlock(rid);"}

    // exited signal
    Deno.stdout.writeSync(new Uint8Array(1));

    // output the enter and exit time
    console.log(JSON.stringify({ enterTime, exitTime }));
`;

  const process = Deno.run({
    cmd: [Deno.execPath(), "eval", "--unstable", scriptText],
    stdout: "piped",
    stdin: "piped",
  });

  const waitSignal = () => process.stdout.read(new Uint8Array(1));
  const signal = () => process.stdin.write(new Uint8Array(1));

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
      const outputBytes = await readAll(process.stdout);
      const text = new TextDecoder().decode(outputBytes);
      return JSON.parse(text) as {
        enterTime: number;
        exitTime: number;
      };
    },
    close: () => {
      process.stdout.close();
      process.stdin.close();
      process.close();
    },
  };
}
