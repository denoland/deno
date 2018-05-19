#!/usr/bin/env node
// Do not include this file from other parts of the code. We use node to
// bootstrap a test runner.

const fs = require("fs");
const path = require("path");
const { spawn, execFileSync } = require("child_process");

// Some tests require an HTTP server. We start one here.
// Because we process tests synchronously in this program we must run
// the server as a subprocess.
// Note that "localhost:4545" is hardcoded into the tests at the moment,
// so if the server runs on a different port, it will fail.
const httpServerArgs = ["node_modules/.bin/http-server", __dirname, "-p 4545"];
const server = spawn(process.execPath, httpServerArgs, {
  cwd: __dirname,
  stdio: "inherit"
});
// TODO: For some reason the http-server doesn't exit properly
// when this program dies. So we force it with the exit handler here.
server.unref();
process.on("exit", () => server.kill("SIGINT"));

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

console.log("Tests done");
