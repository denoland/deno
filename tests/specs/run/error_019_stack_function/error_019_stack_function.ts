function foo(): never {
  throw new Error("function");
}

try {
  foo();
} catch (error) {
  if (error instanceof Error) {
    console.log(error.stack);
  }
  throw error;
}
