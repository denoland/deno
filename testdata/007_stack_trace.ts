function foo() {
  throw Error("bad");
}

function bar() {
  foo()
}

console.log("before");
bar()
console.log("after");
