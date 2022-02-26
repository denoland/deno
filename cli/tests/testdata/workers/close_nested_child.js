// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

console.log("Starting the child worker");

setTimeout(() => {
  console.log("The child worker survived the death of the parent!!!");
}, 2000);
