const p = Promise.all([
  Promise.resolve(),
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
