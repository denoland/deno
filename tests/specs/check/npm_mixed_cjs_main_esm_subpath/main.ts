// Pins the chosen behavior of `exports_has_import_condition`: because the
// `./esm` subpath declares an `import` condition, the package-global heuristic
// flips the CJS main's `.d.ts` to ESM, so ESM-style syntax in that file is
// type-checked as ESM. This imprecision is documented on the heuristic.
import main from "npm:@denotest/mixed-cjs-esm-subpath";
import { value } from "npm:@denotest/mixed-cjs-esm-subpath/esm";

const result: string = main();
const sub: string = value;
console.log(result, sub);
