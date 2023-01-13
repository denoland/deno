import mod from "npm:@denotest/conditional-exports";
import client from "npm:@denotest/conditional-exports/client";
import clientFoo from "npm:@denotest/conditional-exports/client/foo";
import clientBar from "npm:@denotest/conditional-exports/client/bar";
import supportsESM from "npm:supports-esm";

console.log(mod);
console.log(client);
console.log(clientFoo);
console.log(clientBar);
console.log(supportsESM);
