// deno-lint-ignore-file

// @deno-types="./type_definitions/foo.d.ts"
import { foo } from "./type_definitions/foo.js";
// @deno-types="./type_definitions/fizz.d.ts"
import "./type_definitions/fizz.js";

import * as qat from "./type_definitions/qat.ts";

console.log(foo);
console.log(fizz);
console.log(qat.qat);
