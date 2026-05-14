// Copyright 2018-2026 the Deno authors. MIT license.
const AsyncFunction = Object.getPrototypeOf(async function () {
  // empty
}).constructor;

const func = new AsyncFunction(
  `return doesNotExist();
    //# sourceURL=empty.eval`,
);

func.call({});
