import { readFileSync } from "node:fs";
import { SourceMap } from "node:module";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Test 1: Constructor validation - rejects non-object payloads
{
  [1, true, "foo"].forEach((invalidArg) => {
    try {
      new SourceMap(invalidArg);
      assert.fail("Should have thrown");
    } catch (e) {
      assert.strictEqual(e.code, "ERR_INVALID_ARG_TYPE");
      assert.ok(e.message.includes("must be of type object"));
    }
  });
  console.log("PASS: constructor validation");
}

// Test 2: SourceMap can be instantiated with Source Map V3 payload
{
  const payload = JSON.parse(
    readFileSync(resolve(__dirname, "disk.map"), "utf8"),
  );
  const lineLengths = readFileSync(resolve(__dirname, "disk.map"), "utf8")
    .replace(/\n$/, "")
    .split("\n")
    .map((l) => l.length);
  const sourceMap = new SourceMap(payload, { lineLengths });
  const { originalLine, originalColumn, originalSource } = sourceMap.findEntry(
    0,
    29,
  );
  assert.strictEqual(originalLine, 2);
  assert.strictEqual(originalColumn, 4);
  assert.ok(originalSource.endsWith("disk.js"));

  // lineLengths match
  const sourceMapLineLengths = sourceMap.lineLengths;
  for (let i = 0; i < sourceMapLineLengths.length; i++) {
    assert.strictEqual(sourceMapLineLengths[i], lineLengths[i]);
  }
  assert.strictEqual(sourceMapLineLengths.length, lineLengths.length);

  // Payload is cloned
  assert.strictEqual(payload.mappings, sourceMap.payload.mappings);
  assert.notStrictEqual(payload, sourceMap.payload);
  assert.strictEqual(payload.sources[0], sourceMap.payload.sources[0]);
  assert.notStrictEqual(payload.sources, sourceMap.payload.sources);

  console.log("PASS: basic source map V3");
}

// Test 3: findOrigin
{
  const payload = JSON.parse(
    readFileSync(resolve(__dirname, "disk.map"), "utf8"),
  );
  const sourceMap = new SourceMap(payload);
  const entry = sourceMap.findEntry(0, 29);
  const { fileName, lineNumber, columnNumber } = sourceMap.findOrigin(1, 30);
  assert.strictEqual(fileName, entry.originalSource);
  assert.strictEqual(lineNumber, 3);
  assert.strictEqual(columnNumber, 6);
  console.log("PASS: findOrigin");
}

// Test 4: Malformed mappings return empty objects
{
  const payload = JSON.parse(
    readFileSync(resolve(__dirname, "disk.map"), "utf8"),
  );
  payload.mappings = ";;;;;;;;;";
  const sourceMap = new SourceMap(payload);
  const result = sourceMap.findEntry(0, 5);
  assert.strictEqual(typeof result, "object");
  assert.strictEqual(Object.keys(result).length, 0);
  const origin = sourceMap.findOrigin(0, 5);
  assert.strictEqual(typeof origin, "object");
  assert.strictEqual(Object.keys(origin).length, 0);
  console.log("PASS: malformed mappings");
}

// Test 5: Index Source Map V3
{
  const payload = JSON.parse(
    readFileSync(resolve(__dirname, "disk-index.map"), "utf8"),
  );
  const sourceMap = new SourceMap(payload);
  const { originalLine, originalColumn, originalSource } = sourceMap.findEntry(
    0,
    29,
  );
  assert.strictEqual(originalLine, 2);
  assert.strictEqual(originalColumn, 4);
  assert.ok(originalSource.endsWith("section.js"));

  // Payload is cloned
  assert.strictEqual(payload.mappings, sourceMap.payload.mappings);
  assert.notStrictEqual(payload, sourceMap.payload);
  console.log("PASS: index source map V3");
}

// Test 6: VLQ known decodings
{
  function makeMinimalMap(column) {
    return {
      sources: ["test.js"],
      mappings: `AAA${column}`,
    };
  }
  const knownDecodings = {
    A: 0,
    B: -2147483648,
    C: 1,
    D: -1,
    E: 2,
    F: -2,
    "+/////D": 2147483647,
    "8/////D": 2147483646,
    "6/////D": 2147483645,
    "4/////D": 2147483644,
    "2/////D": 2147483643,
    "0/////D": 2147483642,
    "//////D": -2147483647,
    "9/////D": -2147483646,
    "7/////D": -2147483645,
    "5/////D": -2147483644,
    "3/////D": -2147483643,
    "1/////D": -2147483642,
  };
  for (const column in knownDecodings) {
    const sourceMap = new SourceMap(makeMinimalMap(column));
    const { originalColumn } = sourceMap.findEntry(0, 0);
    assert.strictEqual(originalColumn, knownDecodings[column]);
  }
  console.log("PASS: VLQ known decodings");
}

// Test 7: Generated columns sorted with negative offsets
{
  function makeMinimalMap(generatedColumns, originalColumns) {
    return {
      sources: ["test.js"],
      mappings: generatedColumns
        .map((g, i) => `${g}AA${originalColumns[i]}`)
        .join(","),
    };
  }
  const sourceMap = new SourceMap(
    makeMinimalMap(["U", "F", "F"], ["A", "E", "E"]),
  );
  assert.strictEqual(sourceMap.findEntry(0, 6).originalColumn, 4);
  assert.strictEqual(sourceMap.findEntry(0, 8).originalColumn, 2);
  assert.strictEqual(sourceMap.findEntry(0, 10).originalColumn, 0);
  console.log("PASS: sorted columns with negative offsets");
}

console.log("\nAll tests passed!");
