import { getValue } from "npm:@denotest/types-exports-import-types";

// should error here
const result: string = getValue();
console.log(result);
