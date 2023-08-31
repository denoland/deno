import { getKind } from "npm:@denotest/dual-cjs-esm@latest"; // test out @latest dist tag
import * as cjs from "npm:@denotest/dual-cjs-esm@latest/cjs/main.cjs";

console.log(getKind());
console.log(cjs.getKind());
console.log(cjs.getSubPathKind());
