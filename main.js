import { getFoo } from "./foo.ts";

async function bar() {
  throw new Error("fail");
}

let i = 1;
setInterval(async () => {
  if (i === 5) {
    // unhandled promise rejection is not shown
    await bar();
  }
  console.log(i++, getFoo());
}, 100);

// addEventListener("unhandledrejection", (e) => {
//   console.log("unhandledrejection", e.reason);
//   e.preventDefault();
// });
