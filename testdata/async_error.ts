
console.log("hello");
const foo = async () => {
  console.log("before error");
  throw Error("error");
}

foo();
console.log("world");
