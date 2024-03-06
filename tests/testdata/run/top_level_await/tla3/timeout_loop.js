export const foo = "foo";

export function delay(ms) {
  return new Promise((res) =>
    setTimeout(() => {
      res();
    }, ms)
  );
}

let i = 0;

async function timeoutLoop() {
  await delay(1000);
  console.log("timeout loop", i);
  i++;
  if (i > 5) {
    return;
  }
  timeoutLoop();
}

timeoutLoop();
