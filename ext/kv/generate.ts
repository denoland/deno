// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import * as esbuild from "https://deno.land/x/esbuild@v0.14.48/wasm.js";

const source = await Deno.readTextFile("./01_db.ts");
const result = await esbuild.transform(
  source,
  {
    loader: "ts",
    target: "es2022",
    banner: source.split("\n")[0] +
      "\n// DO NOT EDIT: generated from 01_db.ts\n",
  },
);
await Deno.writeTextFile("./01_db.js", result.code);

const status = await new Deno.Command(Deno.execPath(), {
  args: ["fmt", "./01_db.js"],
}).spawn().status;
if (!status.success) {
  throw new Error("deno fmt failed");
}

console.log({ warnings: result.warnings });
Deno.exit(0);
