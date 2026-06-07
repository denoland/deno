// `my-adder` and `adder-caret` are pnpm-style aliases that resolve to the
// scoped `@denotest/adder` workspace member via the `workspace:<name>@<range>`
// form. `my-mult` aliases an unscoped member with an explicit version.
import { add as add1 } from "my-adder";
import { add as add2 } from "adder-caret";
import { multiply } from "my-mult";

console.log("1 + 2 =", add1(1, 2));
console.log("3 + 4 =", add2(3, 4));
console.log("3 * 4 =", multiply(3, 4));
