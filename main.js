import { getFoo } from "./foo.js";

let i = 0;
setInterval(() => console.log(i++, getFoo()));
