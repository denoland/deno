export class Foo {
  method() {
    return Math.random();
  }
}

// This will be analyzed because the method above is missing an
// explicit type which is required for the subset type graph to take
// effect. So the entire source file will be type checked against,
// causing a type error here.
const invalidTypeCheck: number = "";
console.log(invalidTypeCheck);
