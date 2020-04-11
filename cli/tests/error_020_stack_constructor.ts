class A {
  constructor() {
    throw new Error("constructor");
  }
}

try {
  new A();
} catch (error) {
  console.log(error.stack);
  throw error;
}
