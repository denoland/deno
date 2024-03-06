let intervalHandle: number;
let firstIntervalPromise: Promise<void>;

addEventListener("load", () => {
  firstIntervalPromise = new Promise((resolve) => {
    let firstIntervalCalled = false;
    intervalHandle = setInterval(() => {
      if (!firstIntervalCalled) {
        resolve();
        firstIntervalCalled = true;
      }
    }, 5);
  });
});

addEventListener("unload", () => {
  clearInterval(intervalHandle);
});

Deno.test("wait", async function () {
  await firstIntervalPromise;
});
