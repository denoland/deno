function foo(): never {
  throw Error("bad");
}

function bar() {
  foo();
}

bar();
