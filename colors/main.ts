// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { styles } from "./styles.ts";

type Styles = { readonly [S in keyof typeof styles]: Color };

type Color = Styles & {
  (str: string): string;
};

const styleStack: string[] = [];

export const color = function color(str: string): string {
  styleStack.reverse();
  while (styleStack.length) {
    const style = styleStack.pop();
    const code = styles[style];
    str = `${code.open}${str.replace(code.closeRe, code.open)}${
      code.close
    }`.replace(/\r?\n/g, `${code.close}$&${code.open}`);
  }
  return str;
} as Color;

for (const style of Object.keys(styles)) {
  Object.defineProperty(color, style, {
    get() {
      styleStack.push(style);
      return color;
    },
    enumerable: true,
    configurable: false
  });
}
