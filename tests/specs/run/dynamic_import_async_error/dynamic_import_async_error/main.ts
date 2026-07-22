try {
  await import("./delayed_error.ts");
} catch (error) {
  if (error instanceof Error) {
    console.log(`Caught: ${error.stack}`);
  }
}
