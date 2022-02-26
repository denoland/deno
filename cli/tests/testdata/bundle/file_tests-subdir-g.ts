const g: number[] = [];

export class G {
  #g!: number[];
  constructor(shared: boolean) {
    if (shared) {
      this.#g = g;
    } else {
      this.#g = [];
    }
  }
}
