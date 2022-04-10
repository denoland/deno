import init, { parse, parse_frag } from "./build/deno-wasm/deno-wasm.js";
import { register } from "./src/parser.ts";

await init();
register(parse, parse_frag);

export * from "./src/api.ts";

