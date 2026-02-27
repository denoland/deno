// Copyright 2018-2025 the Deno authors. MIT license.
// The handler is set up before this main module is executed
globalThis.onmainmodule = (main) => {
  console.log(main);
};

export const b = 2;

export default {
  a: 1,
};
