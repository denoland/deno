function foo(): never {
  throw Error("bad");
}

function bar(): void {
  foo();
}

bar();
