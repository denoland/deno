(async (): Promise<void> => {
  const { printHello } = await import("../mod2.ts");
  printHello();
})();

export {};
