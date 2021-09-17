// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, unitTest } from "./test_util.ts";
import { readAllSync } from "../../../test_util/std/io/util.ts";

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
  const filePath = "cli/tests/testdata/fixture.json";

  await checkExclusiveLock();
  await checkNonExclusiveLock();

  async function checkExclusiveLock() {
    const fileLock = createFileLock({
      filePath,
      exclusive: true,
      sync: opts.sync,
    });
    try {
      await fileLock.waitEnterLock();
      await assertHasExclusiveLock(filePath, fileLock.pid, true);
    } finally {
      await fileLock.close();
    }
  }

  async function checkNonExclusiveLock() {
    const fileLock1 = createFileLock({
      filePath,
      exclusive: false,
      sync: opts.sync,
    });
    const fileLock2 = createFileLock({
      filePath,
      exclusive: false,
      sync: opts.sync,
    });

    try {
      // both should enter
      await fileLock1.waitEnterLock();
      await fileLock2.waitEnterLock();

      await assertHasExclusiveLock(filePath, fileLock1.pid, false);
      await assertHasExclusiveLock(filePath, fileLock2.pid, false);
    } finally {
      fileLock1.close();
      fileLock2.close();
    }
  }
}

async function assertHasExclusiveLock(
  filePath: string,
  pid: number,
  exclusive: boolean,
) {
  if (Deno.build.os === "windows") {
    assertEquals(
      checkFileCanRead(filePath),
      !exclusive,
      exclusive ? "exclusive cannot read" : "non-exclusive can read",
    );
    assertEquals(
      checkFileCanWrite(filePath),
      false,
      "cannot write",
    );
  } else {
    const process = await Deno.run({
      cmd: ["lsof", "-p", pid.toString()],
      "stdout": "piped",
    });
    try {
      const output = new TextDecoder().decode(await process.output());
      const line = output.split("\n").find((l) => l.includes(filePath));
      assert(line != null, "should find lsof line for process");
      if (exclusive) {
        assert(/\b[0-9]+rW\b/.test(line), "should have exclusive read");
      } else {
        assert(/\b[0-9]+rR\b/.test(line), "should have non-exclusive read");
      }
    } finally {
      process.close();
    }
  }
}

function checkFileCanRead(filePath: string) {
  try {
    const file = Deno.openSync(filePath, { read: true });
    try {
      readAllSync(file);
    } finally {
      file.close();
    }
    return true;
  } catch {
    return false;
  }
}

function checkFileCanWrite(filePath: string) {
  try {
    Deno.openSync(filePath, { write: true }).close();
    return true;
  } catch {
    return false;
  }
}

function createFileLock(opts: {
  filePath: string;
  exclusive: boolean;
  sync: boolean;
}) {
  const scriptText = `
    const { rid } = Deno.openSync("${opts.filePath}");

    ${
    opts.sync
      ? `Deno.flockSync(rid, ${opts.exclusive ? "true" : "false"});`
      : `await Deno.flock(rid, ${opts.exclusive ? "true" : "false"});`
  }
    // signal that we've entered the lock
    Deno.stdout.writeSync(new Uint8Array(1));

    // wait for exit lock signal
    Deno.stdin.readSync(new Uint8Array(1));

    // release the lock
    ${opts.sync ? "Deno.funlockSync(rid);" : "await Deno.funlock(rid);"}

    // signal that we've released the lock
    Deno.stdout.writeSync(new Uint8Array(1));
`;

  const process = Deno.run({
    cmd: [Deno.execPath(), "eval", "--unstable", scriptText],
    stdout: "piped",
    stdin: "piped",
  });

  return {
    get pid() {
      return process.pid;
    },
    async waitEnterLock() {
      await process.stdout.read(new Uint8Array(1));
    },
    async exitLock() {
      await process.stdin.write(new Uint8Array(1));
    },
    async waitExitLock() {
      await process.stdout.read(new Uint8Array(1));
    },
    async close() {
      try {
        await this.exitLock();
      } catch {
        // ignore if already closed
      }
      process.stdout.close();
      process.stdin.close();
      process.close();
    },
  };
}
