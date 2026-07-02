import { getValue, setValue } from "@denotest/esm-basic";
import { hello } from "my-log/other.mjs";
// A jsr dependency declared in package.json as `npm:@jsr/...` maps verbatim.
import { add } from "adder";

setValue(42);
console.log(getValue());
console.log(hello());
console.log(add(1, 2));
