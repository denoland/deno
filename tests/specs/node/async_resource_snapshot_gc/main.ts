// Copyright 2018-2026 the Deno authors. MIT license.
import {
  AsyncLocalStorage,
  AsyncResource,
  executionAsyncResource,
} from "node:async_hooks";
import { strict as assert } from "node:assert";

const gc = (globalThis as unknown as { gc: () => void }).gc;
const storage = new AsyncLocalStorage<{ value: string }>();
const predecessors: WeakRef<AsyncResource>[] = [];

// Model a suspended fiber capturing its next resource before leaving the
// previous resource's scope. Only the final resource should remain reachable.
function captureChain() {
  let resource = new AsyncResource("start");
  for (let i = 0; i < 2048; i++) {
    predecessors.push(new WeakRef(resource));
    resource = resource.runInAsyncScope(
      () => new AsyncResource("resume"),
      null,
    );
  }
  return resource;
}

const current = storage.run({ value: "captured" }, captureChain);
// A resource's reference to its own snapshot must not prevent collection.
const discarded = new WeakRef(new AsyncResource("discarded"));
for (let i = 0; i < 5; i++) {
  await new Promise((resolve) => setTimeout(resolve, 0));
  gc();
}

assert.equal(predecessors.filter((ref) => ref.deref() !== undefined).length, 0);
assert.equal(discarded.deref(), undefined);
// Captured ALS values must stay strongly reachable, even after destruction,
// and the resource must remain reusable, including across asynchronous work.
current.emitDestroy();
await current.runInAsyncScope(async () => {
  assert.equal(storage.getStore()?.value, "captured");
  assert.equal(executionAsyncResource(), current);
  await Promise.resolve();
  assert.equal(storage.getStore()?.value, "captured");
  assert.equal(executionAsyncResource(), current);
}, null);
assert.equal(storage.getStore(), undefined);
console.log("ok");
