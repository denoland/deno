class A {
  m(): never {
    throw new Error("method");
  }
}

try {
  new A().m();
} catch (error) {
  console.log(error.stack);
  throw error;
}
