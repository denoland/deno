import { log } from "./util.ts";

export default class Timer {
  #startTime: number = new Date().getTime();
  #name?: string;
  constructor(name: string) {
    this.#name = name;
  }
  end(): void {
    const endTime = new Date().getTime();
    const seconds = (endTime - this.#startTime) / 1000;
    log(`${this.#name ?? "Time"}: ${seconds.toFixed(2)}s`);
  }
}
