class Frac {
  constructor(num, den) {
    this.num = num;
    this.den = den;
  }
  [Symbol.toPrimitive]() {
    return this.num / this.den;
  }
}
console.log("%f", new Frac(1, 2));
