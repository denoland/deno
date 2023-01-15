// We don't support typescript files in npm packages because we don't
// want to encourage people distributing npm packages that aren't JavaScript.
import { getValue } from "npm:@denotest/typescript-file";

console.log(getValue());
