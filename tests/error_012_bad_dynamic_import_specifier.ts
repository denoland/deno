(async (): Promise<void> => {
  // eslint-disable-next-line
  const badModule = await import("bad-module.ts");
})();
