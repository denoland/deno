export class Foo {
  method(): number {
    return Math.random();
  }
}

// this won't be type checked against because the subset
// type graph will ignore it
const invalidTypeCheck: number = "";
console.log(invalidTypeCheck);
