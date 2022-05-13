// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const res = await fetch(
  "http://localhost:4545/045_mod.ts",
);
console.log(`Response http: ${await res.text()}`);
