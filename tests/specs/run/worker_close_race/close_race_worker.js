// Copyright 2018-2025 the Deno authors. MIT license.

setTimeout(() => {
  self.postMessage("");
  self.close();
}, 500);
