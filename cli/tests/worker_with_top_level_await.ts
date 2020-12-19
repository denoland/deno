// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

function delay(seconds: number): Promise<void> {
  return new Promise<void>((resolve) => {
    setTimeout(() => {
      resolve();
    }, seconds);
  });
}

self.onmessage = (e: MessageEvent) => {
  console.log("TLA worker received message", e.data);
};

self.postMessage("hello");

await delay(3000);

throw new Error("unreachable");
