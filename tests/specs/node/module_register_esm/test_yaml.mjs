// Tests register() with a custom file type loader (the exact scenario
// from https://github.com/denoland/deno/issues/23201).
// The hook module runs in a worker thread, avoiding the deadlock that
// occurred when hook modules were loaded on the main thread.
import { register } from "node:module";

register("./hooks-yaml.mjs", import.meta.url);

const mod = await import("./test_data.yaml");
console.log("greeting:", mod.default.greeting);
console.log("name:", mod.default.name);
console.log("done");
