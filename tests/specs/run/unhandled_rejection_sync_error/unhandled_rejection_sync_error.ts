globalThis.addEventListener("unhandledrejection", (e) => {
  console.log("unhandled rejection at:", e.promise, "reason:", e.reason);
  e.preventDefault();
});

throw new Error("boom!");
