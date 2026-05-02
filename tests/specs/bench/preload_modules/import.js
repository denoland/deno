console.log("import.js starts loading");

setTimeout(() => {
  globalThis.__import__ = "import.js";
  console.log("import.js finished loading");
}, 500);
