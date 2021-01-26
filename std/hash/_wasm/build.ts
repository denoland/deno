// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import { encode as base64Encode } from "../../encoding/base64.ts";

// 1. build wasm
async function buildWasm(path: string): Promise<void> {
  const cmd = [
    "wasm-pack",
    "build",
    "--target",
    "web",
    "--release",
    "-d",
    path,
  ];
  const builder = Deno.run({ cmd });
  const status = await builder.status();

  if (!status.success) {
    console.error(`Failed to build wasm: ${status.code}`);
    Deno.exit(1);
  }
}

// 2. encode wasm
async function encodeWasm(wasmPath: string): Promise<string> {
  const wasm = await Deno.readFile(`${wasmPath}/deno_hash_bg.wasm`);
  return base64Encode(wasm);
}

// 3. generate script
async function generate(wasm: string, output: string): Promise<void> {
  const initScript = await Deno.readTextFile(`${output}/deno_hash.js`);
  const denoHashScript =
    "// deno-lint-ignore-file\n" +
    "//deno-fmt-ignore-file\n" +
    "//deno-lint-ignore-file\n" +
    `import * as base64 from "../../encoding/base64.ts";` +
    `export const source = base64.decode("${wasm}");` +
    initScript;

  await Deno.writeFile("wasm.js", new TextEncoder().encode(denoHashScript));
}

const OUTPUT_DIR = "./out";

await buildWasm(OUTPUT_DIR);
const wasm = await encodeWasm(OUTPUT_DIR);
await generate(wasm, OUTPUT_DIR);
