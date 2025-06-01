import pc from "picocolors";

export function add(a: number, b: number): number {
  return a + b;
}

export function getValue(): string {
  return pc.green("hey");
}
