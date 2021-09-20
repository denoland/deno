// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals, unitTest } from "./test_util.ts";
import { readAll } from "../../../test_util/std/io/util.ts";

unitTest(
  { perms: { read: true, run: true, hrtime: true } },
  async function flockFileSync() {
    await runFlockTests({ sync: true });
  },
);

unitTest(
  { perms: { read: true, run: true, hrtime: true } },
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
    }),
    false,
    "shared does not block shared",
  );
}

async function checkFirstBlocksSecond(opts: {
  firstExclusive: boolean;
  secondExclusive: boolean;
  sync: boolean;
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

    // wait for both processes to signal that they're ready
    await Promise.all([firstProcess.waitSignal(), secondProcess.waitSignal()]);

    // signal to the first process to enter the lock
    await firstProcess.signal();
    await firstProcess.waitSignal(); // entering signal
    await firstProcess.waitSignal(); // entered signal
    // signal the second to enter the lock
    await secondProcess.signal();
    await secondProcess.waitSignal(); // entering signal
    await sleep(100);
    // signal to the first to exit the lock
    await firstProcess.signal();
    // collect the final output so we know it's exited the lock
    const firstPsTimes = await firstProcess.getTimes();
    // signal to the second to exit the lock
    await secondProcess.waitSignal(); // entered signal
    await secondProcess.signal();
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

    // output the enter and exit time
    console.log(JSON.stringify({ enterTime, exitTime }));
`;

  const process = Deno.run({
    cmd: [Deno.execPath(), "eval", "--unstable", scriptText],
    stdout: "piped",
    stdin: "piped",
  });

  return {
    waitSignal: () => process.stdout.read(new Uint8Array(1)),
    signal: () => process.stdin.write(new Uint8Array(1)),
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
