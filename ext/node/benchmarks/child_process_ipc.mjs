// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { fork } from "node:child_process";
import process from "node:process";
import { setImmediate } from "node:timers";

if (process.env.CHILD) {
  const len = +process.env.CHILD;
  const msg = ".".repeat(len);
  let waiting = false;
  const send = () => {
    while (
      process.send(msg, undefined, undefined, (_e) => {
        if (waiting) {
          waiting = false;
          setImmediate(send);
        }
      })
    );
    // Wait: backlog of unsent messages exceeds threshold
    // once the message is sent, the callback will be called
    // and we'll resume
    waiting = true;
  };
  send();
} else {
  function main(dur, len) {
    const p = new Promise((resolve) => {
      const start = performance.now();

      const options = {
        __proto__: null,
        "stdio": ["inherit", "inherit", "inherit", "ipc"],
        "env": { "CHILD": len.toString() },
      };
      const path = new URL("child_process_ipc.mjs", import.meta.url).pathname;
      const child = fork(
        path,
        options,
      );

      let bytes = 0;
      let total = 0;
      child.on("message", (msg) => {
        bytes += msg.length;
        total += 1;
      });

      setTimeout(() => {
        child.kill();
        const end = performance.now();
        const mb = bytes / 1024 / 1024;
        const sec = (end - start) / 1000;
        const mbps = mb / sec;
        console.log(`${len} bytes: ${mbps.toFixed(2)} MB/s`);
        console.log(`${total} messages`);
        resolve();
      }, dur * 1000);
    });
    return p;
  }

  const len = [
    64,
    256,
    1024,
    4096,
    16384,
    65536,
    65536 << 4,
    65536 << 6 - 1,
  ];

  for (const l of len) {
    await main(5, l);
  }
}
