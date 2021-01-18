// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
const res = await fetch("http://localhost:4545/std/examples/colors.ts");
console.log(`Response http: ${await res.text()}`);
