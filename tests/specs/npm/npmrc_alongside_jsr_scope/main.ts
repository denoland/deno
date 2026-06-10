import { add } from "npm:@jsr/denotest__add";
import { getValue, setValue } from "npm:@denotest/basic";

console.log(add(1, 2));
setValue(1);
console.log(getValue());
