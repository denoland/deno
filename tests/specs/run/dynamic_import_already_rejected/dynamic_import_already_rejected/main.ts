try {
  await import("./error_001.ts");
} catch (error) {
  if (error instanceof Error) {
    console.log(`Caught: ${error.stack}`);
  }
}

try {
  await import("./error_001.ts");
} catch (error) {
  if (error instanceof Error) {
    console.log(`Caught: ${error.stack}`);
  }
}
