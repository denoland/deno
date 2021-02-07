// Copyright 2020 the Deno authors. All rights reserved. MIT license.

const dataSmall = "";
const dataLarge = "x".repeat(10 * 1024);

onmessage = function (e): void {
  for (let i = 0; i <= 10; i++) {
    if (i % 2 == 0) {
      postMessage(dataLarge);
    } else {
      postMessage(dataSmall);
    }
  }
};
