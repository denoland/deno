try {
  await import("http://localhost:4545/this_module_does_not_exist.ts");
} catch (err) {
  console.log("caught:", (err as Error).message);
}
