// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

setTimeout(() => {
  self.postMessage("");
  self.close();
}, 500);
