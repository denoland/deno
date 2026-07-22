class A {
  m(): never {
    throw new Error("method");
  }
}

try {
  new A().m();
} catch (error) {
  if (error instanceof Error) {
    console.log(error.stack);
  }
  throw error;
}
