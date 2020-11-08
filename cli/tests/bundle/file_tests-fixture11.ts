import { a as defaultA, O } from "./subdir/m.ts";
export { O } from "./subdir/m.ts";

interface AOptions {
  a?(): void;
  c?: O;
}

class A {
  #a: () => void;
  #c?: O;
  constructor(o: AOptions = {}) {
    const {
      a = defaultA,
      c,
    } = o;
    this.#a = a;
    this.#c = c;
  }

  a() {
    this.#a();
  }

  c() {
    console.log(this.#c);
  }
}

const a = new A();
a.a();
a.c();
