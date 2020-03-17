console.log("hello");
const foo = (): Promise<never> => {
  console.log("before error");
  throw Error("error");
};

foo();
console.log("world");
