// Regression test: an ESM module (inside an npm package) that
// default-imports a sibling `.cjs` file. The compile-only
// `import.meta.main` transform used to parse the generated ESM facade
// under `MediaType::Cjs` (script mode) and fail with a parse error.
import fn from "@denotest/esm-import-cjs-default";

console.log("result", fn());
