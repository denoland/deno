const sleep = (n) => new Promise((r) => setTimeout(r, n));

window.addEventListener("load", () => {
  console.log("module loaded");
});

console.log("before sleep");
await sleep(5000);

console.log("after sleep");

export default 1;

// ▶ deno run -A test1.js
// before sleep
// after sleep
// 1
