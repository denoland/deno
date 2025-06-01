let interval: number | null = null;
addEventListener("load", () => {
  if (interval) {
    throw new Error("Interval is already set");
  }

  console.log("load");
  interval = setInterval(() => {}, 0);
});

addEventListener("unload", () => {
  if (!interval) {
    throw new Error("Interval was not set");
  }

  console.log("unload");
  clearInterval(interval);
});

Deno.test("test", () => {
  console.log("test");
  if (!interval) {
    throw new Error("Interval was not set");
  }
});
