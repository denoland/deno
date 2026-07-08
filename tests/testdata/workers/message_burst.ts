// Copyright 2018-2026 the Deno authors. MIT license.

// On receiving a count, synchronously posts that many messages back to the
// host in one event-loop turn. This exercises the host-side (Web `Worker`
// main-thread) bounded sync-drain receive loop: the messages all land in the
// channel as a burst, so the main thread receives the first asynchronously and
// drains the rest synchronously.
self.onmessage = (e: MessageEvent) => {
  const count = e.data as number;
  for (let i = 0; i < count; i++) {
    self.postMessage(i);
  }
};
