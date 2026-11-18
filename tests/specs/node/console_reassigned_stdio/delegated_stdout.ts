// Copyright 2018-2026 the Deno authors. MIT license.

import { deepStrictEqual, strictEqual } from "node:assert";
import process, { stdout } from "node:process";

const output: string[] = [];
stdout.write = function (this: typeof stdout, chunk) {
  strictEqual(this, process.stdout);
  output.push(String(chunk));
  return true;
};

console.log("delegated %d", 42);
deepStrictEqual(output, ["delegated 42\n"]);
