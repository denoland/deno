import { exported_add } from "./import.wasm";

Deno.core.print(`${exported_add()}\n`);
