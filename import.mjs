import { exported_add } from "./import.wasm";

Deno.core.print(String(exported_add()));
