// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

setTimeout(() => {
  self.postMessage("");
  self.close();
}, 500);
