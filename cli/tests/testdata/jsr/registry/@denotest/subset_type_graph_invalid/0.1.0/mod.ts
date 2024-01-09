export class Foo {
  method() {
    return Math.random();
  }
}

// this will be analyzed because the method above is missing an
// explicit type which is required for the subset type graph
const invalidTypeCheck: number = "";
console.log(invalidTypeCheck);
