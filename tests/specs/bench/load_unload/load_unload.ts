let interval: NodeJS.Timeout | null = null;
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

Deno.bench("bench", () => {
  if (!interval) {
    throw new Error("Interval was not set");
  }
});
