import { printHello } from "./subdir/mod2.ts";

export class Foo {
  /**
   * Create a Foo, with some markdown
   *
   *     new Foo("bar");
   *
   */
  constructor(public bar: string) {
    printHello();
  }

  log(): void {
    console.log("foo");
  }
}
