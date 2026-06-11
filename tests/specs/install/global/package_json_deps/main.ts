import { getValue, setValue } from "@denotest/esm-basic";
import { hello } from "my-log/other.mjs";

setValue(42);
console.log(getValue());
console.log(hello());
