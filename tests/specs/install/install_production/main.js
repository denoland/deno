import { getValue, setValue } from "@denotest/esm-basic";
import nestedValue from "@denotest/different-nested-dep";
if (nestedValue !== 1) throw new Error("Missing shared production dependency");
setValue(42);
console.log(getValue());
