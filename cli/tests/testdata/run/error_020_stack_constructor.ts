class A {
  constructor() {
    throw new Error("constructor");
  }
}

try {
  new A();
} catch (error) {
  if (error instanceof Error) {
    console.log(error.stack);
  }
  throw error;
}
