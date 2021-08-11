try {
  await import("./error_001.ts");
} catch (error) {
  console.log(`Caught: ${error.stack}`);
}

try {
  await import("./error_001.ts");
} catch (error) {
  console.log(`Caught: ${error.stack}`);
}
