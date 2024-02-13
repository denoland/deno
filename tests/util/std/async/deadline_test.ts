// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertRejects } from "../assert/mod.ts";
import { delay } from "./delay.ts";
import { deadline, DeadlineError } from "./deadline.ts";

Deno.test("[async] deadline: return fulfilled promise", async () => {
  const controller = new AbortController();
  const { signal } = controller;
  const p = delay(100, { signal })
    .catch(() => {})
    .then(() => "Hello");
  const result = await deadline(p, 1000);
  assertEquals(result, "Hello");
  controller.abort();
});

Deno.test("[async] deadline: throws DeadlineError", async () => {
  const controller = new AbortController();
  const { signal } = controller;
  const p = delay(1000, { signal })
    .catch(() => {})
    .then(() => "Hello");
  await assertRejects(async () => {
    await deadline(p, 100);
  }, DeadlineError);
  controller.abort();
});

Deno.test("[async] deadline: thrown when promise is rejected", async () => {
  const controller = new AbortController();
  const { signal } = controller;
  const p = delay(100, { signal })
    .catch(() => {})
    .then(() => Promise.reject(new Error("booom")));
  await assertRejects(
    async () => {
      await deadline(p, 1000);
    },
    Error,
    "booom",
  );
  controller.abort();
});

Deno.test("[async] deadline: with non-aborted signal", async () => {
  const controller = new AbortController();
  const { signal } = controller;
  const p = delay(100, { signal })
    .catch(() => {})
    .then(() => "Hello");
  const abort = new AbortController();
  const result = await deadline(p, 1000, { signal: abort.signal });
  assertEquals(result, "Hello");
  controller.abort();
});

Deno.test("[async] deadline: with signal aborted after delay", async () => {
  const controller = new AbortController();
  const { signal } = controller;
  const p = delay(100, { signal })
    .catch(() => {})
    .then(() => "Hello");
  const abort = new AbortController();
  const promise = deadline(p, 100, { signal: abort.signal });
  abort.abort();
  await assertRejects(async () => {
    await promise;
  }, DeadlineError);
  controller.abort();
});

Deno.test("[async] deadline: with already aborted signal", async () => {
  const controller = new AbortController();
  const { signal } = controller;
  const p = delay(100, { signal })
    .catch(() => {})
    .then(() => "Hello");
  const abort = new AbortController();
  abort.abort();
  await assertRejects(async () => {
    await deadline(p, 100, { signal: abort.signal });
  }, DeadlineError);
  controller.abort();
});
