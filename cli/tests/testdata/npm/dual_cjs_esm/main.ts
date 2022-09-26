import { getKind } from "npm:@denotest/dual-cjs-esm";
import * as cjs from "npm:@denotest/dual-cjs-esm/cjs/main.cjs";

console.log(getKind());
console.log(cjs.getKind());
console.log(cjs.getSubPathKind());
