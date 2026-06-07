import { findSourceMap, SourceMap } from "node:module";
import { writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";
import assert from "node:assert/strict";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// Test 1: findSourceMap returns undefined for files without source maps
{
  const files = [__filename, "", "invalid-file"];
  for (const file of files) {
    const sourceMap = findSourceMap(file);
    assert.strictEqual(sourceMap, undefined);
  }
  console.log("PASS: findSourceMap returns undefined for non-mapped files");
}

// Test 2: findSourceMap can look up external source maps
// Write a minified file that references disk.map
{
  const minifiedPath = resolve(__dirname, "minified_test.js");
  const content =
    'class Foo{constructor(x=33){this.x=x?x:99;if(this.x){console.info("covered")}else{console.info("uncovered")}this.methodC()}methodA(){console.info("covered")}methodB(){console.info("uncovered")}methodC(){console.info("covered")}methodD(){console.info("uncovered")}}const a=new Foo(0);const b=new Foo(33);a.methodA();\n//# sourceMappingURL=./disk.map';
  writeFileSync(minifiedPath, content);

  const sourceMap = findSourceMap(minifiedPath);
  assert.ok(sourceMap instanceof SourceMap);
  const { originalLine, originalColumn, originalSource } = sourceMap.findEntry(
    0,
    29,
  );
  assert.strictEqual(originalLine, 2);
  assert.strictEqual(originalColumn, 4);
  assert.ok(originalSource.endsWith("disk.js"));

  const { fileName, lineNumber, columnNumber } = sourceMap.findOrigin(1, 30);
  assert.strictEqual(fileName, originalSource);
  assert.strictEqual(lineNumber, 3);
  assert.strictEqual(columnNumber, 6);

  assert.ok(Array.isArray(sourceMap.lineLengths));
  assert.ok(!sourceMap.lineLengths.some((len) => typeof len !== "number"));
  console.log("PASS: findSourceMap with external source map");
}

// Test 3: findSourceMap with inline base64 source map
{
  const inlinePath = resolve(__dirname, "inline_test.js");
  const payload = JSON.stringify({
    version: 3,
    sources: ["original.js"],
    names: [],
    mappings: "AAAA",
    sourceRoot: "",
  });
  const b64 = btoa(payload);
  writeFileSync(
    inlinePath,
    `console.log("hello");\n//# sourceMappingURL=data:application/json;base64,${b64}`,
  );

  const sourceMap = findSourceMap(inlinePath);
  assert.ok(sourceMap instanceof SourceMap);
  assert.ok(sourceMap.payload);
  assert.ok(Array.isArray(sourceMap.payload.sources));
  assert.strictEqual(sourceMap.payload.sources[0], "original.js");
  console.log("PASS: findSourceMap with inline base64 source map");
}

// Test 4: findSourceMap caches results
{
  const minifiedPath = resolve(__dirname, "minified_test.js");
  const sm1 = findSourceMap(minifiedPath);
  const sm2 = findSourceMap(minifiedPath);
  assert.strictEqual(sm1, sm2);
  console.log("PASS: findSourceMap caches results");
}

console.log("\nAll findSourceMap tests passed!");
