// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

setTimeout(() => {
  self.postMessage("");
  self.close();
}, 500);
