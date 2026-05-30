// Copyright 2018-2026 the Deno authors. MIT license.

import { aborted } from "node:util";

declare const gc: (options?: object) => void;

const signals: AbortSignal[] = [];
let finalizedCount = 0;
const registry = new FinalizationRegistry(() => finalizedCount++);
const count = 100;

function createPendingAbortedPromise() {
  const resource = {};
  const signal = AbortSignal.timeout(2 ** 31 - 1);
  const promise = aborted(signal, resource);
  registry.register(promise, undefined);
  signals.push(signal);
}

for (let i = 0; i < count; i++) {
  createPendingAbortedPromise();
}

function delay() {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

for (let i = 0; i < 30 && finalizedCount < count / 2; i++) {
  gc({ type: "major", execution: "sync" });
  await delay();
}

if (finalizedCount < count / 2) {
  throw new Error(
    `Expected pending aborted promises to be collectable, got ${finalizedCount}/${count}`,
  );
}
