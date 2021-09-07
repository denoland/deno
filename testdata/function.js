export function foo() {
  console.log("foo");
  bar();
}

function bar() {
  console.log("bar");
}

function baz() {
  console.log("baz");
}

foo();
