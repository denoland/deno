// Test that setting `self` in the main thread to some other value doesn't break
// the world, in particular for events fired on the global scope.

// deno-lint-ignore no-global-assign
self = null;

addEventListener("load", () => {
  console.log("load event (event listener)");
});

addEventListener("unload", () => {
  console.log("unload event (event listener)");
});

globalThis.onload = () => {
  console.log("load event (event handler)");
};

globalThis.onunload = () => {
  console.log("unload event (event handler)");
};
