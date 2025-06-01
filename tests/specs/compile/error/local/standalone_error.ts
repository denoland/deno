function boom() {
  throw new Error("boom!");
}

function foo() {
  boom();
}

foo();
