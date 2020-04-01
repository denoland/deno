(async () => {
  const { printHello } = await import("../print_hello.ts");
  printHello();
})();

export {};
