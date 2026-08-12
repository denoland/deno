import { getValue, setValue } from "npm:@denotest/esm-basic";

setValue(42);
console.log("esm", getValue());
