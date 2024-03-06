import { D, d } from "./subdir/q.ts";

class A {
  private s: D = d();

  a() {
    this.s.resolve();
  }
}

new A();
