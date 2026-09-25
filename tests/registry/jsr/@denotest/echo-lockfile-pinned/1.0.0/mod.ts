import { sum } from "jsr:@denotest/add@^0.2.0";

console.log(sum(1, 2), ...Deno.args);
