// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

console.log("Starting the parent worker");

new Worker(
  new URL("./close_nested_child.js", import.meta.url),
  { type: "module" },
);

self.addEventListener("message", () => {
  console.log("Closing");
  self.close();
});
