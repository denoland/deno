// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
function hello() {
  console.log("Hello from extension!");
}
globalThis.Extension = { hello };
