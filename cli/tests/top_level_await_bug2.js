const mod = await import("./top_level_await_bug_nested.js");
console.log(mod);

const sleep = (n) => new Promise((r) => setTimeout(r, n));

await sleep(100);
console.log("slept");

window.addEventListener("load", () => {
  console.log("load event");
});

setTimeout(() => {
  console.log("timeout");
}, 1000);
