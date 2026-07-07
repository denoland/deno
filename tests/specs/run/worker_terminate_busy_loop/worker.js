// Copyright 2018-2026 the Deno authors. MIT license.

setTimeout(() => {
  postMessage("looping");
  while (true) {
    Date.now();
  }
}, 0);
