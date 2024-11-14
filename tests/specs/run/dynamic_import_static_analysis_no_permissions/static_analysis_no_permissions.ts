try {
  await import("./empty_1.ts");
  console.log("✅ Succeeded importing statically analyzable specifier");
} catch {
  console.log("❌ Failed importing statically analyzable specifier");
}

try {
  await import("" + "./empty_2.ts");
  console.log("❌ Succeeded importing non-statically analyzable specifier");
} catch {
  console.log("✅ Failed importing non-statically analyzable specifier");
}
