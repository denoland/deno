// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const WS = typeof WebSocket !== "undefined" ? WebSocket : require("ws");
const websocket = new WS("ws://localhost:8000");

const msg = "hello";
function bench() {
  return new Promise((resolve) => {
    websocket.onmessage = (e) => {
      resolve();
    };
    websocket.send(msg);
  });
}

async function run() {
  const start = performance.now();
  let count = 0;
  let bytes = 0;
  while (performance.now() - start < 1000) {
    await bench();
    count++;
    bytes += msg.length;
  }
  console.log(
    `Sent`,
    count,
    `messages in 1 sec, throughput: ${bytes} bytes/sec`,
  );
}

websocket.onopen = run;
