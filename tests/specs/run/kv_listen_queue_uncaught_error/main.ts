// Copyright 2018-2026 the Deno authors. MIT license.

// Regression test for https://github.com/denoland/deno/issues/20464
// Opening a KV, attaching listenQueue() without await, and then hitting an
// uncaught error on the main task used to panic the runtime with
// "Attempted to use a closed database" while the queue's dequeue loop raced
// against the dropped SQLite connection. The error path must now unwind
// cleanly: surface the JS error, exit non-zero, no panic.

const kv = await Deno.openKv(":memory:");
kv.listenQueue(() => {});
console.log("Listening for messages...");

// Mirror the reporter's "Cannot access 'manifest' before initialization"
// — any uncaught error on the main task is enough to expose the race.
throw new Error("kaboom from main");
