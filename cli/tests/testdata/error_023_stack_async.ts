const p = (async () => {
  await Promise.resolve().then((): never => {
    throw new Error("async");
  });
})();

try {
  await p;
} catch (error) {
  if (error instanceof Error) {
    console.log(error.stack);
  }
  throw error;
}
