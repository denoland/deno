import type { Config } from "./types.ts";

export class Client {
  #config: Config;
  constructor(config: Config) {
    this.#config = config;
  }

  getTimeout(): number {
    return this.#config.timeout;
  }
}

export function createClient(config: Config): Client {
  return new Client(config);
}
