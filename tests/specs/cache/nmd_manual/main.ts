import { add } from "@denotest/add";
import { getValue } from "@denotest/esm-basic";

import { sub } from "jsr:@denotest/subtract";

console.log(add(1, 2));
console.log(getValue());
console.log(sub(1, 2));
