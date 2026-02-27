console.log("lib.js before");

export function sleep(timeout) {
  return new Promise((resolve) => {
    Deno.core.queueUserTimer(
      Deno.core.getTimerDepth() + 1,
      false,
      timeout,
      resolve,
    );
  });
}
await sleep(100);

console.log("lib.js after");

const abc = 1 + 2;
export function add(a, b) {
  console.log(`abc: ${abc}`);
  return a + b;
}
