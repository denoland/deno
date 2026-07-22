import { getValue, setValue } from "npm:@denotest/basic";

console.log(getValue());
setValue(42);
console.log(getValue());
