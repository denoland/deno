// add some statements that will be removed by the subset
// type graph so that we can test that the source map works
console.log(1);
console.log(2);
console.log(3);

export class Foo {
  method(): number {
    return Math.random();
  }
}

// this won't be type checked against because the subset
// type graph omit this code because it's not part of the
// public API.
const invalidTypeCheck: number = "";
console.log(invalidTypeCheck);
