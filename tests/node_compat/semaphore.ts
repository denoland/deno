export class Semaphore {
  #usedPermits = 0;
  #maxPermits: number;
  #queue: Array<() => void> = [];

  constructor(maxConcurrency: number) {
    if (maxConcurrency < 1) {
      throw new Error("maxConcurrency must be at least 1");
    }
    this.#maxPermits = maxConcurrency;
  }

  async acquire(): Promise<void> {
    if (this.#usedPermits < this.#maxPermits) {
      this.#usedPermits++;
      return Promise.resolve();
    }

    return new Promise<void>((resolve) => {
      this.#queue.push(resolve);
    });
  }

  release(): void {
    const resolve = this.#queue.shift();
    if (resolve) {
      resolve();
    } else {
      this.#usedPermits--;
    }
  }

  async run<T>(fn: () => Promise<T>): Promise<T> {
    await this.acquire();
    try {
      return await fn();
    } finally {
      this.release();
    }
  }

  setMaxConcurrency(newLimit: number): void {
    if (newLimit < 1) {
      throw new Error("maxConcurrency must be at least 1");
    }
    this.#maxPermits = newLimit;
  }
}
