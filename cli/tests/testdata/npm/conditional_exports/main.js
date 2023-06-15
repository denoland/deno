import mod from "npm:@denotest/conditional-exports";
import foo from "npm:@denotest/conditional-exports/foo.js";
import client from "npm:@denotest/conditional-exports/client";
import clientFoo from "npm:@denotest/conditional-exports/client/foo";
import clientBar from "npm:@denotest/conditional-exports/client/bar";
import clientM from "npm:@denotest/conditional-exports/client/m";
import supportsESM from "npm:supports-esm";

console.log(mod);
console.log(foo);
console.log(client);
console.log(clientFoo);
console.log(clientBar);
console.log(clientM);
console.log(supportsESM);
