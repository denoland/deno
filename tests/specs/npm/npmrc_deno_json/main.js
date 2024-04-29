import { getValue, setValue } from "npm:@denotest/esm-basic";

console.log(getValue());
setValue(42);
console.log(getValue());
