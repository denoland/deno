import { foo } from "./foo.jsx";

let i = 0;
setInterval(() => {
  console.log(i++, foo());
}, 100);
