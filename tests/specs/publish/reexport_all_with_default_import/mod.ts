// Regression test for https://github.com/denoland/deno/issues/28293
// `export *` excludes the default export, but the module is also default
// imported here. Both traces must be merged without dropping the default,
// otherwise the generated fast check declaration loses `export default` and
// publishing fails type checking with TS2613.
import Bad from "./class_bad.ts";
export * from "./class_bad.ts";
export default Bad;
export { Bad };
