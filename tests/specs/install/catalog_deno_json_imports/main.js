import { getValue, setValue } from "@denotest/esm-basic";
import { add } from "@denotest/add";
setValue(add(40, 2));
console.log(getValue());
