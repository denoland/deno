// Regression test for denoland/deno#25349.
//
// When running under the inspector, Deno pads TypeScript output after transpile
// so the emitted JavaScript keeps source-mapped tokens on their original source
// lines. Tools such as the Chrome DevTools performance profiler report the raw
// V8 line numbers without applying source maps, so a function's reported
// location must line up with its position in the `.ts` source.
//
// `[[FunctionLocation]]` (read via the inspector's Runtime/Debugger domains) is
// exactly the raw, un-source-mapped location V8 records for a function — the same
// information the profiler's flame graph uses.

import inspector from "node:inspector/promises";
import { strict as assert } from "node:assert";
import { targetFunction, throwingFunction } from "./worker.ts";

// Expose the transpiled function so we can look up its location over the
// inspector protocol.
globalThis.__targetFunction = targetFunction;

// Figure out the line `targetFunction` is declared on in the original source.
const workerUrl = new URL("./worker.ts", import.meta.url);
const sourceLines = (await Deno.readTextFile(workerUrl)).split("\n");
const expectedLine = sourceLines.findIndex((line) =>
  line.includes("export function targetFunction")
);
assert.ok(expectedLine > 0, "could not find targetFunction in the source");

const session = new inspector.Session();
session.connect();

await session.post("Runtime.enable");
await session.post("Debugger.enable");

const { result } = await session.post("Runtime.evaluate", {
  expression: "globalThis.__targetFunction",
});
assert.ok(result.objectId, "expected an objectId for the function");

const { internalProperties } = await session.post("Runtime.getProperties", {
  objectId: result.objectId,
  ownProperties: false,
});

const functionLocation = internalProperties?.find(
  (prop) => prop.name === "[[FunctionLocation]]",
)?.value?.value;
assert.ok(
  functionLocation,
  "expected a [[FunctionLocation]] internal property",
);

// `lineNumber` is zero-based, matching `Array#findIndex` above.
assert.equal(
  functionLocation.lineNumber,
  expectedLine,
  `expected targetFunction at source line ${
    expectedLine + 1
  }, but V8 reports ` +
    `line ${functionLocation.lineNumber + 1}`,
);

session.disconnect();

// Make sure the function still actually runs.
assert.equal(targetFunction(5), 10);

const expectedThrowLine =
  sourceLines.findIndex((line) =>
    line.includes('throw new Error("line check")')
  ) + 1;
assert.ok(expectedThrowLine > 0, "could not find throwing line in the source");
try {
  throwingFunction(1);
  assert.fail("expected throwingFunction to throw");
} catch (error) {
  assert.match(
    error.stack,
    new RegExp(`worker\\.ts:${expectedThrowLine}:`),
    "expected stack trace to still use the original source line",
  );
}

console.log("PASS: function line number matches source");
