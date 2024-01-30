// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

Deno.test(function checkExposedOps() {
  // @ts-ignore TS doesn't allow to index with symbol
  const core = Deno[Deno.internal].core;
  const opNames = Object.keys(core.ops);

  if (opNames.length !== 200) {
    throw new Error(
      `Expected 200 ops, but got ${opNames.length}:\n${opNames.join("\n")}`,
    );
  }
});
