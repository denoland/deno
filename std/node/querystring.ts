interface ParseOptions {
  decodeURIComponent: (string: string) => string;
  maxKeys: number;
}

export function parse(
  str: string,
  sep: string = "&",
  eq: string = "=",
  { decodeURIComponent = unescape, maxKeys = 1000 }: ParseOptions = {}
): any {
  let entries = str
    .split(sep)
    .map(entry => entry.split(eq).map(decodeURIComponent));
  let final: any = {};

  let i = 0;
  while (true) {
    if ((Object.keys(final).length === maxKeys && !!maxKeys) || !entries[i]) {
      break;
    }

    const [key, val] = entries[i];

    if (final[key]) {
      if (Array.isArray(final[key])) {
        final[key].push(val);
      } else {
        final[key] = [final[key], val];
      }
    } else {
      final[key] = val;
    }

    i++;
  }

  return final;
}

interface StringifyOptions {
  encodeURIComponent: (string: string) => string;
}

export function stringify(
  obj: object,
  sep: string = "&",
  eq: string = "=",
  { encodeURIComponent = escape }: StringifyOptions = {}
): string {
  let final = [];

  for (const entry of Object.entries(obj)) {
    if (Array.isArray(entry[1])) {
      for (const val of entry[1]) {
        final.push(encodeURIComponent(entry[0]) + eq + encodeURIComponent(val));
      }
    } else if (typeof entry[1] !== "object" && entry[1] !== undefined) {
      final.push(entry.map(encodeURIComponent).join(eq));
    } else {
      final.push(encodeURIComponent(entry[0]) + eq);
    }
  }

  return final.join(sep);
}

export const decode = parse;
export const encode = stringify;
export const unescape = decodeURIComponent;
export const escape = encodeURIComponent;
