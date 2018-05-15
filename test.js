#!/usr/bin/env node
// Do not include this file from other parts of the code. We use node to
// bootstrap a test runner.

const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

const testdataDir = path.join(__dirname, "testdata");
const denoFn = path.join(__dirname, "deno");
const files = fs
  .readdirSync(testdataDir)
  .filter(fn => fn.endsWith(".out"))
  .map(fn => path.join(testdataDir, fn));

function deno(inFile) {
  let args = [inFile];
  console.log("deno", ...args);
  return execFileSync(denoFn, args);
}

for (const outFile of files) {
  const inFile = outFile.replace(/\.out$/, "");
  let stdoutBuffer = deno(inFile);
  let outFileBuffer = fs.readFileSync(outFile);
  if (0 != Buffer.compare(stdoutBuffer, outFileBuffer)) {
    throw Error(`test error
--- stdoutBuffer - ${inFile}
${stdoutBuffer.toString()}
--- outFileBuffer - ${outFile}
${outFileBuffer.toString()}
---------------------
    `);
  }
}
