// Copyright 2018-2026 the Deno authors. MIT license.

setTimeout(() => {
  self.postMessage("");
  self.close();
}, 500);
