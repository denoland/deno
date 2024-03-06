const g: number[] = [];

export class H {
  #g!: number[];
  constructor(shared: boolean) {
    if (shared) {
      this.#g = g;
    } else {
      this.#g = [];
    }
  }
}
