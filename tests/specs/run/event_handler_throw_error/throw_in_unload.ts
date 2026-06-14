globalThis.addEventListener("unload", () => {
  throw new Error("unload error");
});
