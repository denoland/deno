import { createRequire } from "node:module";

// Import this so that deno_graph knows to download this file.
if (false) import("npm:@denotest/lossy-utf8-script@1.0.0");

const require = createRequire(import.meta.url);

const mod = require("@denotest/lossy-utf8-script");

console.log(mod);
