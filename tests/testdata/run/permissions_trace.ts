function foo() {
  Deno.hostname();
}

function bar() {
  foo();
}

bar();
