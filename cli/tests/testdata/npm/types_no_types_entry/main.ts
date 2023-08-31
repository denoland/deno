import { getValue } from "npm:@denotest/types-no-types-entry";

// should error here
const result: string = getValue();
console.log(result);
