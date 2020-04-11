const p = (async (): Promise<void> => {
  await Promise.resolve().then((): never => {
    throw new Error("async");
  });
})();

try {
  await p;
} catch (error) {
  console.log(error.stack);
  throw error;
}
