import { getValue, setValue } from "@denotest/esm-basic";

setValue(42);
console.log("value:", getValue());
