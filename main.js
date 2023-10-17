import { getFoo } from "./foo.ts";

let i = 0;
setInterval(() => {
  if (i === 5) {
    // Uncaught exception isn't shown in the terminal and
    // it breaks watch + hmr
    console.log("Before 123throw");
    throw new Error("fail");
  }
  console.log(i++, getFoo());
}, 250);
