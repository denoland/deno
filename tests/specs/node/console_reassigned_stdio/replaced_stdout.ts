// Copyright 2018-2026 the Deno authors. MIT license.

import { deepStrictEqual } from "node:assert";
import process from "node:process";
import { Writable } from "node:stream";

const stdout = process.stdout;
const output: string[] = [];
const replacement = new Writable({
  write(chunk, _encoding, callback) {
    output.push(String(chunk));
    callback();
  },
});

Object.assign(process, { stdout: replacement });
try {
  console.log("replacement %d", 42);
} finally {
  process.stdout = stdout;
}

deepStrictEqual(output, ["replacement 42\n"]);
console.log("restored");
