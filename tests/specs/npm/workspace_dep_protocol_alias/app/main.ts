// `my-adder` and `adder-caret` are pnpm-style aliases that resolve to the
// `@denotest/adder` workspace member via the `workspace:<name>@<range>` form.
import { add as add1 } from "my-adder";
import { add as add2 } from "adder-caret";

console.log("1 + 2 =", add1(1, 2));
console.log("3 + 4 =", add2(3, 4));
