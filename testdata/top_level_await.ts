import { printHello } from "./subdir/print_hello.ts";

await printHello();

import * as deno from "deno";

const p = new Promise(res => {
  setTimeout(() => {
    res(42);
  }, 100);
});

console.log("the meaning of live, the universe and everything is:");

const answer = await p;

console.log(answer);
