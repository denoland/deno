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
  if (error instanceof Error) {
    console.log(error.stack);
  }
  throw error;
}
