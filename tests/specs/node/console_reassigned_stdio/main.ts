// Copyright 2018-2026 the Deno authors. MIT license.

import { deepStrictEqual, strictEqual } from "node:assert";
import process from "node:process";

const { stdout, stderr } = process;
const writeStdout = stdout.write;
const writeStderr = stderr.write;
const output: string[] = [];
const errors: string[] = [];

stdout.write = function (this: typeof stdout, chunk) {
  strictEqual(this, stdout);
  output.push(String(chunk));
  return true;
};
stderr.write = function (this: typeof stderr, chunk) {
  strictEqual(this, stderr);
  errors.push(String(chunk));
  return true;
};

try {
  console.log("log %d", 42);
  console.error("error %d", 42);
} finally {
  stdout.write = writeStdout;
  stderr.write = writeStderr;
}

deepStrictEqual(output, ["log 42\n"]);
deepStrictEqual(errors, ["error 42\n"]);
console.log("restored");
