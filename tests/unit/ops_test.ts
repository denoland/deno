// Copyright 2018-2026 the Deno authors. MIT license.

const EXPECTED_OP_COUNT = 41;
const EXPECTED_WORKER_OP_COUNT = 19;

function getExposedOpNames(): string[] {
  // @ts-ignore TS doesn't allow to index with symbol
  const core = Deno[Deno.internal].core;
  return Object.keys(core.ops);
}

Deno.test(function checkExposedOps() {
  const opNames = getExposedOpNames();

  if (opNames.length !== EXPECTED_OP_COUNT) {
    throw new Error(
      `Expected ${EXPECTED_OP_COUNT} ops, but got ${opNames.length}:\n${
        opNames.join("\n")
      }`,
    );
  }
});
Deno.test(function internalCoreOnlyHidesExtensionLoaders() {
  // @ts-ignore TS doesn't allow to index with symbol
  const core = Deno[Deno.internal].core;

  for (const name of ["createLazyLoader", "loadExtScript"]) {
    if (name in core) {
      throw new Error(`${name} should not be exposed`);
    }
  }

  for (const name of ["close", "read", "readAll"]) {
    if (typeof core[name] !== "function") {
      throw new Error(`${name} should remain exposed`);
    }
  }
});

Deno.test(async function workerDoesNotExposeImportedOps() {
  const mainOpNames = getExposedOpNames();
  const worker = new Worker(
    `data:application/javascript,${
      encodeURIComponent(`
        // @ts-ignore TS doesn't allow to index with symbol
        const core = Deno[Deno.internal].core;
        postMessage(Object.keys(core.ops));
      `)
    }`,
    { type: "module" },
  );
  let actualOpNames: string[];
  try {
    actualOpNames = await new Promise((resolve, reject) => {
      worker.onmessage = (event) => resolve(event.data);
      worker.onerror = (event) => reject(event.error);
    });
  } finally {
    worker.terminate();
  }
  if (
    actualOpNames.length !== EXPECTED_WORKER_OP_COUNT ||
    actualOpNames.some((opName) => !mainOpNames.includes(opName))
  ) {
    throw new Error(
      `Unexpected worker ops:\n${actualOpNames.join("\n")}`,
    );
  }
});
