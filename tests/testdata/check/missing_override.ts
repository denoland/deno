export class Base {
  method() {
  }
}

export class Derived extends Base {
  // missing override keyword
  method() {
  }
}
