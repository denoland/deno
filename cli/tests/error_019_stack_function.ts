function foo(): never {
  throw new Error("function");
}

try {
  foo();
} catch (error) {
  console.log(error.stack);
  throw error;
}
