// Simulates an npm package bundle where source files don't exist
function throwError() {
  throw new Error("Test error from bundle with missing source");
}

throwError();

//# sourceMappingURL=bundle.js.map
