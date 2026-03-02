import { readFile } from "node:fs/promises";
import { WASI } from "node:wasi";

const wasi = new WASI({
  version: "preview1",
  args: ["hello"],
  env: {},
});

const wasm = await WebAssembly.compile(
  await readFile(new URL("./wasi_hello.wasm", import.meta.url)),
);
const instance = await WebAssembly.instantiate(wasm, wasi.getImportObject());
const exitCode = wasi.start(instance);
console.log("exit code:", exitCode);
