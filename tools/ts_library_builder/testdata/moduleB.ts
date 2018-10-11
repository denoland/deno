/** jsdoc about Foo */
export class Foo {
  private _foo = "foo";
  /** jsdoc about Foo.log() */
  log() {
    console.log(this._foo);
    return this._foo;
  }
}
