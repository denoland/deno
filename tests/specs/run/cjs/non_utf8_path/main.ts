try {
  await import("./bad-%FF/module.cjs");
  console.log("unexpected success");
} catch (error) {
  if (
    !(error instanceof Error) ||
    !error.message.includes("CommonJS module path") ||
    !error.message.includes("not valid UTF-8")
  ) {
    throw error;
  }
  console.log("caught");
}
