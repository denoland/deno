let interval: number | null = null;
addEventListener("load", () => {
  if (interval) {
    throw new Error("Interval is already set");
  }

  interval = setInterval(() => {}, 0);
});

addEventListener("unload", () => {
  if (!interval) {
    throw new Error("Interval was not set");
  }

  clearInterval(interval);
});

Deno.test("test", () => {
  if (!interval) {
    throw new Error("Interval was not set");
  }
});
