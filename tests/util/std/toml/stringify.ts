// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

// Bare keys may only contain ASCII letters,
// ASCII digits, underscores, and dashes (A-Za-z0-9_-).
function joinKeys(keys: string[]): string {
  // Dotted keys are a sequence of bare or quoted keys joined with a dot.
  // This allows for grouping similar properties together:
  return keys
    .map((str: string): string => {
      return str.length === 0 || str.match(/[^A-Za-z0-9_-]/)
        ? JSON.stringify(str)
        : str;
    })
    .join(".");
}

enum ArrayType {
  ONLY_PRIMITIVE,
  ONLY_OBJECT_EXCLUDING_ARRAY,
  MIXED,
}

export interface FormatOptions {
  keyAlignment?: boolean;
}

class Dumper {
  maxPad = 0;
  srcObject: Record<string, unknown>;
  output: string[] = [];
  #arrayTypeCache = new Map<unknown[], ArrayType>();
  constructor(srcObjc: Record<string, unknown>) {
    this.srcObject = srcObjc;
  }
  dump(fmtOptions: FormatOptions = {}): string[] {
    // deno-lint-ignore no-explicit-any
    this.output = this.#printObject(this.srcObject as any);
    this.output = this.#format(fmtOptions);
    return this.output;
  }
  #printObject(obj: Record<string, unknown>, keys: string[] = []): string[] {
    const out = [];
    const props = Object.keys(obj);
    const inlineProps = [];
    const multilineProps = [];
    for (const prop of props) {
      if (this.#isSimplySerializable(obj[prop])) {
        inlineProps.push(prop);
      } else {
        multilineProps.push(prop);
      }
    }
    const sortedProps = inlineProps.concat(multilineProps);
    for (let i = 0; i < sortedProps.length; i++) {
      const prop = sortedProps[i];
      const value = obj[prop];
      if (value instanceof Date) {
        out.push(this.#dateDeclaration([prop], value));
      } else if (typeof value === "string" || value instanceof RegExp) {
        out.push(this.#strDeclaration([prop], value.toString()));
      } else if (typeof value === "number") {
        out.push(this.#numberDeclaration([prop], value));
      } else if (typeof value === "boolean") {
        out.push(this.#boolDeclaration([prop], value));
      } else if (
        value instanceof Array
      ) {
        const arrayType = this.#getTypeOfArray(value);
        if (arrayType === ArrayType.ONLY_PRIMITIVE) {
          out.push(this.#arrayDeclaration([prop], value));
        } else if (arrayType === ArrayType.ONLY_OBJECT_EXCLUDING_ARRAY) {
          // array of objects
          for (let i = 0; i < value.length; i++) {
            out.push("");
            out.push(this.#headerGroup([...keys, prop]));
            out.push(...this.#printObject(value[i], [...keys, prop]));
          }
        } else {
          // this is a complex array, use the inline format.
          const str = value.map((x) => this.#printAsInlineValue(x)).join(",");
          out.push(`${this.#declaration([prop])}[${str}]`);
        }
      } else if (typeof value === "object") {
        out.push("");
        out.push(this.#header([...keys, prop]));
        if (value) {
          const toParse = value as Record<string, unknown>;
          out.push(...this.#printObject(toParse, [...keys, prop]));
        }
        // out.push(...this._parse(value, `${path}${prop}.`));
      }
    }
    out.push("");
    return out;
  }
  #isPrimitive(value: unknown): boolean {
    return value instanceof Date ||
      value instanceof RegExp ||
      ["string", "number", "boolean"].includes(typeof value);
  }
  #getTypeOfArray(arr: unknown[]): ArrayType {
    if (this.#arrayTypeCache.has(arr)) {
      return this.#arrayTypeCache.get(arr)!;
    }
    const type = this.#doGetTypeOfArray(arr);
    this.#arrayTypeCache.set(arr, type);
    return type;
  }
  #doGetTypeOfArray(arr: unknown[]): ArrayType {
    if (!arr.length) {
      // any type should be fine
      return ArrayType.ONLY_PRIMITIVE;
    }

    const onlyPrimitive = this.#isPrimitive(arr[0]);
    if (arr[0] instanceof Array) {
      return ArrayType.MIXED;
    }
    for (let i = 1; i < arr.length; i++) {
      if (
        onlyPrimitive !== this.#isPrimitive(arr[i]) || arr[i] instanceof Array
      ) {
        return ArrayType.MIXED;
      }
    }
    return onlyPrimitive
      ? ArrayType.ONLY_PRIMITIVE
      : ArrayType.ONLY_OBJECT_EXCLUDING_ARRAY;
  }
  #printAsInlineValue(value: unknown): string | number {
    if (value instanceof Date) {
      return `"${this.#printDate(value)}"`;
    } else if (typeof value === "string" || value instanceof RegExp) {
      return JSON.stringify(value.toString());
    } else if (typeof value === "number") {
      return value;
    } else if (typeof value === "boolean") {
      return value.toString();
    } else if (
      value instanceof Array
    ) {
      const str = value.map((x) => this.#printAsInlineValue(x)).join(",");
      return `[${str}]`;
    } else if (typeof value === "object") {
      if (!value) {
        throw new Error("should never reach");
      }
      const str = Object.keys(value).map((key) => {
        return `${joinKeys([key])} = ${
          // deno-lint-ignore no-explicit-any
          this.#printAsInlineValue((value as any)[key])}`;
      }).join(",");
      return `{${str}}`;
    }

    throw new Error("should never reach");
  }
  #isSimplySerializable(value: unknown): boolean {
    return (
      typeof value === "string" ||
      typeof value === "number" ||
      typeof value === "boolean" ||
      value instanceof RegExp ||
      value instanceof Date ||
      (value instanceof Array &&
        this.#getTypeOfArray(value) !== ArrayType.ONLY_OBJECT_EXCLUDING_ARRAY)
    );
  }
  #header(keys: string[]): string {
    return `[${joinKeys(keys)}]`;
  }
  #headerGroup(keys: string[]): string {
    return `[[${joinKeys(keys)}]]`;
  }
  #declaration(keys: string[]): string {
    const title = joinKeys(keys);
    if (title.length > this.maxPad) {
      this.maxPad = title.length;
    }
    return `${title} = `;
  }
  #arrayDeclaration(keys: string[], value: unknown[]): string {
    return `${this.#declaration(keys)}${JSON.stringify(value)}`;
  }
  #strDeclaration(keys: string[], value: string): string {
    return `${this.#declaration(keys)}${JSON.stringify(value)}`;
  }
  #numberDeclaration(keys: string[], value: number): string {
    switch (value) {
      case Infinity:
        return `${this.#declaration(keys)}inf`;
      case -Infinity:
        return `${this.#declaration(keys)}-inf`;
      default:
        return `${this.#declaration(keys)}${value}`;
    }
  }
  #boolDeclaration(keys: string[], value: boolean): string {
    return `${this.#declaration(keys)}${value}`;
  }
  #printDate(value: Date): string {
    function dtPad(v: string, lPad = 2): string {
      return v.padStart(lPad, "0");
    }
    const m = dtPad((value.getUTCMonth() + 1).toString());
    const d = dtPad(value.getUTCDate().toString());
    const h = dtPad(value.getUTCHours().toString());
    const min = dtPad(value.getUTCMinutes().toString());
    const s = dtPad(value.getUTCSeconds().toString());
    const ms = dtPad(value.getUTCMilliseconds().toString(), 3);
    // formatted date
    const fData = `${value.getUTCFullYear()}-${m}-${d}T${h}:${min}:${s}.${ms}`;
    return fData;
  }
  #dateDeclaration(keys: string[], value: Date): string {
    return `${this.#declaration(keys)}${this.#printDate(value)}`;
  }
  #format(options: FormatOptions = {}): string[] {
    const { keyAlignment = false } = options;
    const rDeclaration = /^(\".*\"|[^=]*)\s=/;
    const out = [];
    for (let i = 0; i < this.output.length; i++) {
      const l = this.output[i];
      // we keep empty entry for array of objects
      if (l[0] === "[" && l[1] !== "[") {
        // non-empty object with only subobjects as properties
        if (
          this.output[i + 1] === "" &&
          this.output[i + 2]?.slice(0, l.length) === l.slice(0, -1) + "."
        ) {
          i += 1;
          continue;
        }
        out.push(l);
      } else {
        if (keyAlignment) {
          const m = rDeclaration.exec(l);
          if (m) {
            out.push(l.replace(m[1], m[1].padEnd(this.maxPad)));
          } else {
            out.push(l);
          }
        } else {
          out.push(l);
        }
      }
    }
    // Cleaning multiple spaces
    const cleanedOutput = [];
    for (let i = 0; i < out.length; i++) {
      const l = out[i];
      if (!(l === "" && out[i + 1] === "")) {
        cleanedOutput.push(l);
      }
    }
    return cleanedOutput;
  }
}

/**
 * Stringify dumps source object into TOML string and returns it.
 * @param srcObj
 * @param [fmtOptions] format options
 * @param [fmtOptions.keyAlignment] whether to algin key
 */
export function stringify(
  srcObj: Record<string, unknown>,
  fmtOptions?: FormatOptions,
): string {
  return new Dumper(srcObj).dump(fmtOptions).join("\n");
}
