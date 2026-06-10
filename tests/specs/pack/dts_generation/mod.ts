export interface Config {
  name: string;
  verbose: boolean;
}

export function greet(config: Config): string {
  return `Hello, ${config.name}!`;
}
