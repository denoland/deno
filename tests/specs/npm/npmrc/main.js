import { getValue, setValue } from "@denotest/esm-basic";
import * as test from "@denotest2/basic";

console.log(getValue());
setValue(42);
console.log(getValue());

console.log(test.getValue());
