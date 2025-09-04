// Copyright 2018-2025 the Deno authors. MIT license.

console.log("Starting the child worker");

setTimeout(() => {
  console.log("The child worker survived the death of the parent!!!");
  Deno.exit(1);
}, 2000);
