// Reproduces the error thrown by the `bindings` npm package (used by libxmljs
// and many other native addons) when the compiled `.node` file is missing
// because npm lifecycle scripts were not run. See denoland/deno#27933.
const tried = [
  "/node_modules/libxmljs/build/xmljs.node",
  "/node_modules/libxmljs/build/Debug/xmljs.node",
  "/node_modules/libxmljs/build/Release/xmljs.node",
];
throw new Error(
  "Could not locate the bindings file. Tried:\n" +
    tried.map((p) => " → " + p).join("\n"),
);
