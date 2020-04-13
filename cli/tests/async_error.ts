console.log("hello");
// eslint-disable-next-line require-await
const foo = async (): Promise<never> => {
  console.log("before error");
  throw Error("error");
};

foo();
console.log("world");
