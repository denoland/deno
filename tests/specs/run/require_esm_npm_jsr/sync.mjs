import { add } from "npm:@denotest/add";
import { add as add2 } from "jsr:@denotest/add";

console.log(add(1, 2));
console.log(add2(1, 2));
