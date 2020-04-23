const p = Promise.all([
  (async (): Promise<never> => {
    await Promise.resolve();
    throw new Error("Promise.all()");
  })(),
]);

try {
  await p;
} catch (error) {
  console.log(error.stack);
  throw error;
}
