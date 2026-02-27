// Copyright 2018-2026 the Deno authors. MIT license.
(async () => {
  const p = (async () => {
    await Promise.resolve().then(() => {
      throw new Error("async");
    });
  })();
  try {
    await p;
  } catch (error) {
    console.log(error.stack);
    throw error;
  }
})();
