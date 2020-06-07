// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { Mutex } from "./mutex.ts";
import { delay as sleep } from "./delay.ts";

let x = 0;
async function needsLockX(): Promise<void> {
  const oldX: number = x;
  await sleep(10);
  x = oldX + 1;
}

async function isLockedX(): Promise<void> {
  await Mutex.doAtomic("x", needsLockX);
}

let y = 0;
async function needsLockY(): Promise<void> {
  const oldY: number = y;
  await sleep(10);
  y = oldY + 1;
}

async function isLockedY(): Promise<void> {
  await Mutex.doAtomic("y", needsLockY);
}

let z = 0;
async function needsLockButThrows(): Promise<void> {
  const zOld = z;
  await sleep(10);
  z = zOld + 1;
  throw new Error("Some error");
}

async function lockFail(): Promise<void> {
  try {
    await Mutex.doAtomic("z", needsLockButThrows);
  } catch (e) {
  }
}

Deno.test("code without locks is subject to race conditions", async function () {
  x = 0;
  await Promise.all([needsLockX(), needsLockX(), needsLockX()]);
  if (x !== 1) {
    throw new Error("Code without locks behaves unexpectedly: x = " + x);
  }
});

Deno.test("doAtomic prevents race conditions", async function () {
  x = 0;
  await Promise.all([isLockedX(), isLockedX(), isLockedX()]);
  if (x !== 3) {
    throw new Error("race condition detected: x = " + x);
  }
});

Deno.test("without locks (2)", async function () {
  x = 0;
  y = 0;
  await Promise.all([
    needsLockY(),
    needsLockX(),
    needsLockY(),
    needsLockX(),
    needsLockY(),
    needsLockX(),
    needsLockY(),
    needsLockY(),
    needsLockY(),
    needsLockY(),
    needsLockX(),
    needsLockX(),
    needsLockY(),
    needsLockY(),
    needsLockY(),
    needsLockY(),
  ]);
  if (x !== 1 || y !== 1) {
    throw new Error(
      "Code without locks behaves unexpectedly: " +
        "x = " + x + ", y = " + y,
    );
  }
});

Deno.test("doAtomic with multiple named locks", async function () {
  x = 0;
  y = 0;
  await Promise.all([
    isLockedY(),
    isLockedX(),
    isLockedY(),
    isLockedX(),
    isLockedY(),
    isLockedX(),
    isLockedY(),
    isLockedY(),
    isLockedY(),
    isLockedY(),
    isLockedX(),
    isLockedX(),
    isLockedY(),
    isLockedY(),
    isLockedY(),
    isLockedY(),
  ]);
  if (x !== 5 || y !== 11) {
    throw new Error("Race condition detected: x = " + x + ", y = " + y);
  }
});

Deno.test("deadlock should not occur if locked function throws", async function () {
  z = 0;
  const multiFailp = Promise.all([lockFail(), lockFail(), lockFail()]);

  let testDonec = function (): void {};

  const testDonep = new Promise<void>(function (res, _) {
    testDonec = res;
  });

  const multiFailOrTimeoutp = new Promise<void>(function (res, rej) {
    multiFailp.then(() => {
      res();
    }).catch(() => {
      rej();
    });
    // 10ms * 3 = should only take ~30ms.  100ms for some breathing room
    sleep(100).then(function () {
      rej(); //will be ignored if the above fired first
      testDonec();
    });
  });

  try {
    await multiFailOrTimeoutp;
  } catch (e) {
    throw new Error("Deadlock Detected");
  }
  if (z !== 3) {
    throw new Error("Race condition detected: z = " + z);
  }

  await testDonep; // wait for timeout to fire before exiting test
});

Deno.test("reusing a lock name should be possible", async function () {
  x = 0;

  await Mutex.doAtomic("x", needsLockX);
  await sleep(100);
  await Mutex.doAtomic("x", needsLockX);

  if (x !== 2) {
    throw new Error("Race condition detected: x = ");
  }
});

Deno.test("reusing a lock should be possible", async function () {
  x = 0;

  //Mutex.doAtomic does some cleanup that could mask errors
  const recycledMu = new Mutex();
  async function recyclingDoAtomic(cb: () => void): Promise<void> {
    await recycledMu.lock();
    try {
      await cb();
    } finally {
      recycledMu.unlock();
    }
  }

  await recyclingDoAtomic(needsLockX);
  await sleep(100);
  await recyclingDoAtomic(needsLockX);

  if (x !== 2) {
    throw new Error("Race condition detected: x = ");
  }
});

Deno.test("should not be allowed to unlock before locking", function () {
  const mu = new Mutex();
  try {
    mu.unlock();
  } catch (e) {
    return;
  }

  throw new Error("no error when releasing nonexistent lock");
});

Deno.test("Should not be allowed to unlock the same lock twice", async function () {
  const mu = new Mutex();
  await mu.lock();
  mu.unlock();
  try {
    mu.unlock();
  } catch (e) {
    return;
  }
  throw new Error("no error when double unlocking");
});

Deno.test("locking twice should deadlock", async function () {
  const mu = new Mutex();
  await mu.lock();
  let done = function (): void {};
  const donep = new Promise<void>(function (res, _) {
    done = res;
  });
  const lockOrTimeoutp = new Promise<void>(function (res, rej) {
    sleep(200).then(() => {
      res();
      done();
    });
    mu.lock()
      .then(() => {
        rej("No deadlock detected");
      })
      .catch(() => {
        rej("No deadlock detected");
      });
  });

  await lockOrTimeoutp;
  await donep;
});
