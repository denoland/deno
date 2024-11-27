async function main() {
  const mod = await import("package");
  const value: "value" = mod.kind;
  console.log(value);
  console.log(mod);
}

main();
