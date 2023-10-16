import { getFoo } from "./foo.js";

//// asdf
let i = 0;
setInterval(() => console.log(i++, getFoo()), 1000);
