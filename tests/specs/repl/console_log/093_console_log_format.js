// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
class Frac {
  constructor(num, den) {
    this.num = num;
    this.den = den;
  }
  [Symbol.toPrimitive]() {
    return this.num / this.den;
  }
  display() {
    console.log(`${this.num / this.den}`);
  }
}
const f = new Frac(1, 2);
f.display();
