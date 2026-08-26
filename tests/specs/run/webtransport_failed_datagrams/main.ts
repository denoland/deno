// Copyright 2018-2026 the Deno authors. All rights reserved. MIT license.

const wt = new WebTransport("https://127.0.0.1:65535");

const reader = wt.datagrams.readable.getReader();
const read = reader.read();

const writer = wt.datagrams.writable.getWriter();
const writes = [];
for (let i = 0; i < 5; i++) {
  writes.push(writer.write(new Uint8Array([i])));
}
const writesSettled = Promise.allSettled(writes);

await wt.ready.then(
  () => console.log("ready unexpectedly resolved"),
  (err) => console.log("ready rejected", err.name),
);

const readResult = await read;
console.log("read done", readResult.done);

const writeResults = await writesSettled;
console.log(
  "write rejected",
  writeResults.some((result) => result.status === "rejected"),
);

await wt.closed;
console.log("survived");
