/** jsdoc for Bar */
export class Bar {
  private _bar: string;
  /** jsdoc for Bar.log() */
  log() {
    console.log(this._bar);
    return this.log;
  }
}

/**
 * jsdoc for qat
 * @param a jsdoc for qat(a)
 * @param b jsdoc for qat(b)
 */
export function qat(a: string, b: string) {
  return a + b;
}
