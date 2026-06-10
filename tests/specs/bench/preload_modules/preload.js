console.log("preload.js starts loading");

setTimeout(() => {
  globalThis.__preload__ = "preload.js";

  console.log("preload.js finished loading");
}, 500);
