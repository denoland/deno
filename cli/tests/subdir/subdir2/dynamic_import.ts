(async (): Promise<void> => {
  const { printHello } = await import("../print_hello.ts");
  printHello();
})();

export {};
