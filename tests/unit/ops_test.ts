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
Deno.test(async function workerDoesNotExposeImportedOps() {
  const worker = new Worker(
    `data:application/javascript,${
      encodeURIComponent(`
        // @ts-ignore TS doesn't allow to index with symbol
        const core = Deno[Deno.internal].core;
        postMessage(typeof core.ops.op_node_ipc_ref);
      `)
    }`,
    { type: "module" },
  );
  const result = await new Promise((resolve, reject) => {
    worker.onmessage = (event) => resolve(event.data);
    worker.onerror = (event) => reject(event.error);
  });
  worker.terminate();
  if (result !== "undefined") {
    throw new Error(`op_node_ipc_ref unexpectedly exposed: ${result}`);
  }
});
