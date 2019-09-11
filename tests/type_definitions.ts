// @deno-types="./type_definitions/foo.d.ts"
import { foo } from "./type_definitions/foo.js";
// @deno-types="./type_definitions/fizz.d.ts"
import "./type_definitions/fizz.js";

console.log(foo);
