// main.ts
import { foo } from "./foo.jsx";

let i = 0;
setInterval(() => {
  console.log(i++, foo());
}, 1000);

// foo.jsx
