// Copyright 2018-2026 the Deno authors. MIT license.

const EXPECTED_OP_COUNT = 41;

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

Deno.test(function createLazyLoaderDoesNotExposeVirtualOpsModule() {
  // @ts-ignore TS doesn't allow to index with symbol
  const core = Deno[Deno.internal].core;

  if (core.ops.op_get_env_no_permission_check !== undefined) {
    throw new Error("op_get_env_no_permission_check should not be exposed");
  }

  try {
    core.createLazyLoader("ext:core/ops")();
  } catch (error) {
    if (!(error instanceof TypeError)) {
      throw error;
    }
    return;
  }

  throw new Error("createLazyLoader unexpectedly exposed ext:core/ops");
});
