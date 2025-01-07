// Copyright 2018-2025 the Deno authors. MIT license.

console.log("Starting the parent worker");

new Worker(
  import.meta.resolve("./close_nested_child.js"),
  { type: "module" },
);

self.addEventListener("message", () => {
  console.log("Closing");
  self.close();
});
