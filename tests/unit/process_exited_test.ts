// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertEquals, assertRejects } from "./test_util.ts";

function spawnHanging(): Deno.ChildProcess {
  // A process that never exits on its own, so the test controls when it dies.
  return new Deno.Command(Deno.execPath(), {
    args: ["eval", "await new Promise(() => {});"],
    stdout: "null",
    stderr: "null",
  }).spawn();
}

Deno.test(
  { permissions: { run: false } },
  async function processExitedPermissions() {
    await assertRejects(
      () => Deno.processExited(Deno.pid),
      Deno.errors.NotCapable,
    );
  },
);

Deno.test(
  { permissions: { run: true } },
  async function processExitedInvalidPid() {
    await assertRejects(() => Deno.processExited(1.5), TypeError);
    await assertRejects(() => Deno.processExited(NaN), TypeError);
    await assertRejects(() => Deno.processExited(0), TypeError);
    await assertRejects(() => Deno.processExited(-1), TypeError);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function processExitedResolvesOnExit() {
    const child = spawnHanging();
    // The wait is registered synchronously (before the first await) while the
    // process is still alive, so terminating it right after is race-free.
    const exited = Deno.processExited(child.pid);
    child.kill("SIGTERM");
    assertEquals(await exited, undefined);
    await child.status;
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function processExitedResolvesWhenAlreadyExited() {
    const child = new Deno.Command(Deno.execPath(), {
      args: ["eval", ""],
      stdout: "null",
      stderr: "null",
    }).spawn();
    const pid = child.pid;
    // Ensure the process is gone (and reaped) before we start waiting.
    await child.status;
    assertEquals(await Deno.processExited(pid), undefined);
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function processExitedNullSignalIsNoSignal() {
    // `{ signal: null }` is a common "no signal" idiom and must not be treated
    // as an (invalid) abort signal.
    const child = spawnHanging();
    // @ts-ignore `null` is accepted at runtime as "no signal".
    const exited = Deno.processExited(child.pid, { signal: null });
    child.kill("SIGTERM");
    assertEquals(await exited, undefined);
    await child.status;
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function processExitedAbort() {
    const child = spawnHanging();
    const ac = new AbortController();
    const exited = Deno.processExited(child.pid, { signal: ac.signal });
    ac.abort();
    const err = await assertRejects(() => exited, DOMException);
    assertEquals(err.name, "AbortError");
    // Aborting the wait must not have killed the process.
    child.kill("SIGTERM");
    await child.status;
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function processExitedAbortWithReason() {
    const child = spawnHanging();
    const ac = new AbortController();
    ac.abort(new Error("stop waiting"));
    await assertRejects(
      () => Deno.processExited(child.pid, { signal: ac.signal }),
      Error,
      "stop waiting",
    );
    child.kill("SIGTERM");
    await child.status;
  },
);

Deno.test(
  { permissions: { run: true, read: true } },
  async function processExitedSurvivesWaitAcrossManyResources() {
    // Opening and completing many waits should not leak descriptors/handles.
    const child = spawnHanging();
    const waits = [];
    for (let i = 0; i < 8; i++) {
      waits.push(Deno.processExited(child.pid));
    }
    child.kill("SIGTERM");
    await Promise.all(waits);
    assert(true);
    await child.status;
  },
);
