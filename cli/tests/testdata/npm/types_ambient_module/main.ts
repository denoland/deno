import * as import1 from "npm:@denotest/types-ambient";
import * as import2 from "npm:@denotest/types-ambient@1";

console.log(import1.Test);
console.log(import1.Test2); // should error
console.log(import2.Test);
console.log(import2.Test2); // should error
