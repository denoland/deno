console.log("hello");
const foo = async (): Promise<never> => {
  console.log("before error");
  throw Error("error");
};

foo();
console.log("world");
