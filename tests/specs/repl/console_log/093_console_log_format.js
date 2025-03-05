// Copyright 2018-2025 the Deno authors. MIT license.
class Frac {
  constructor(num, den) {
    this.num = num;
    this.den = den;
  }
  [Symbol.toPrimitive]() {
    return this.num / this.den;
  }
  display() {
    const result = this.num / this.den;
    process.stdout.write(`${result}`);
  }
}
const f = new Frac(1, 2);
f.display();
