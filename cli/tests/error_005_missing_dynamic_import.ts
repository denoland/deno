(async (): Promise<void> => {
  const _badModule = await import("./bad-module.ts");
})();
