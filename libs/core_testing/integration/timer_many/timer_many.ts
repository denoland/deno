// Copyright 2018-2026 the Deno authors. MIT license.
let n = 0;
for (let i = 0; i < 1e6; i++) setTimeout(() => n++, 1);
setTimeout(() => console.log(n), 2);
