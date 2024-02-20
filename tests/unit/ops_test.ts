// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

const EXPECTED_OP_COUNT = 11;

Deno.test(function checkExposedOps() {
  // @ts-ignore TS doesn't allow to index with symbol
  const core = Deno[Deno.internal].core;
  const opNames = Object.keys(core.ops);

  if (opNames.length !== EXPECTED_OP_COUNT) {
    throw new Error(
      `Expected ${EXPECTED_OP_COUNT} ops, but got ${opNames.length}:\n${
        opNames.join("\n")
      }`,
    );
  }
});
