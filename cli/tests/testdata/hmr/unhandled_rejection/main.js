import { foo } from "./foo.jsx";

// deno-lint-ignore require-await
async function rejection() {
  throw new Error("boom!");
}

let i = 0;
setInterval(() => {
  if (i == 3) {
    rejection();
  }
  console.log(i++, foo());
}, 100);
