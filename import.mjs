import { exported_add } from "./import.wasm";
Deno.core.print(`hey ${exported_add}\n`);

Deno.core.print(`${exported_add()}\n`);
