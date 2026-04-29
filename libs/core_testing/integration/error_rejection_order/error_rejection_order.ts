// Copyright 2018-2026 the Deno authors. MIT license.
for (let i = 0; i < 100; i++) {
  Promise.reject(i);
}
