import { add } from "@denotest/add";
import { getValue } from "@denotest/esm-basic";

import { subtract } from "jsr:@denotest/subtract";

console.log(add(3, 2));
console.log(getValue());
console.log(subtract(3, 2));
