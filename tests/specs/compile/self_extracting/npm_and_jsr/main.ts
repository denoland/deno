import { add } from "jsr:@denotest/add@1.0.0";
import { url } from "npm:@denotest/esm-basic";
import { printHello } from "http://localhost:4545/subdir/print_hello.ts";

console.log(`add(1, 2) = ${add(1, 2)}`);
console.log(`url = ${typeof url}`);
printHello();
console.log("success");
