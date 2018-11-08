// Fake headers to work around
// https://github.com/denoland/deno/issues/1173

function normalize(name: string, value?: string): [string, string] {
  name = String(name).toLowerCase();
  value = String(value).trim();
  return [name, value];
}

export class Headers {
  private map = new Map<string, string>();

  get(name: string): string | null {
    let [name_] = normalize(name);
    return this.map.get(name_);
  }

  append(name: string, value: string): void {
    [name, value] = normalize(name, value);
    this.map.set(name, value);
  }

  toString(): string {
    let out = "";
    this.map.forEach((v, k) => {
      out += `${k}: ${v}\n`;
    });
    return out;
  }

  [Symbol.iterator](): IterableIterator<[string, string]> {
    return this.map[Symbol.iterator]();
  }
}
