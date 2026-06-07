globalThis.addEventListener("beforeunload", () => {
  throw new Error("beforeunload error");
});
