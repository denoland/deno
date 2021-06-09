try {
  await import("./delayed_error.ts");
} catch (error) {
  console.log(`Caught: ${error.stack}`);
}
