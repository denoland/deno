(async () => {
  const { printHello } = await import("../mod2.ts");
  printHello();
})();
