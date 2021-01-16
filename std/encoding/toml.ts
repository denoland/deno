// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { deepAssign } from "../_util/deep_assign.ts";
import { assert } from "../_util/assert.ts";

class TOMLError extends Error {}

class KeyValuePair {
  constructor(public key: string, public value: unknown) {}
}

class ParserGroup {
  arrValues: unknown[] = [];
  objValues: Record<string, unknown> = {};

  constructor(public type: string, public name: string) {}
}

class ParserContext {
  currentGroup?: ParserGroup;
  output: Record<string, unknown> = {};
}

class Parser {
  tomlLines: string[];
  context: ParserContext;
  constructor(tomlString: string) {
    this.tomlLines = this._split(tomlString);
    this.context = new ParserContext();
  }
  _sanitize(): void {
    const out: string[] = [];
    for (let i = 0; i < this.tomlLines.length; i++) {
      const s = this.tomlLines[i];
      const trimmed = s.trim();
      if (trimmed !== "") {
        out.push(s);
      }
    }
    this.tomlLines = out;
    this._removeComments();
    this._mergeMultilines();
  }

  _removeComments(): void {
    function isFullLineComment(line: string) {
      return line.match(/^#/) ? true : false;
    }

    function stringStart(line: string) {
      const m = line.match(/(?:=\s*\[?\s*)("""|'''|"|')/);
      if (!m) {
        return false;
      }

      // We want to know which syntax was used to open the string
      openStringSyntax = m[1];
      return true;
    }

    function stringEnd(line: string) {
      // match the syntax used to open the string when searching for string close
      // e.g. if we open with ''' we must close with a '''
      const reg = RegExp(`(?<!(=\\s*))${openStringSyntax}(?!(.*"))`);
      if (!line.match(reg)) {
        return false;
      }

      openStringSyntax = "";
      return true;
    }

    const cleaned = [];
    let isOpenString = false;
    let openStringSyntax = "";
    for (let i = 0; i < this.tomlLines.length; i++) {
      const line = this.tomlLines[i];

      // stringStart and stringEnd are separate conditions to
      // support both single-line and multi-line strings
      if (!isOpenString && stringStart(line)) {
        isOpenString = true;
      }
      if (isOpenString && stringEnd(line)) {
        isOpenString = false;
      }

      if (!isOpenString && !isFullLineComment(line)) {
        const out = line.split(
          /(?<=([\,\[\]\{\}]|".*"|'.*'|\w(?!.*("|')+))\s*)#/gi,
        );
        cleaned.push(out[0].trim());
      } else if (isOpenString || !isFullLineComment(line)) {
        cleaned.push(line);
      }

      // If a single line comment doesnt end on the same line, throw error
      if (
        isOpenString && (openStringSyntax === "'" || openStringSyntax === '"')
      ) {
        throw new TOMLError(`Single-line string is not closed:\n${line}`);
      }
    }

    if (isOpenString) {
      throw new TOMLError(`Incomplete string until EOF`);
    }

    this.tomlLines = cleaned;
  }

  _mergeMultilines(): void {
    function arrayStart(line: string): boolean {
      const reg = /.*=\s*\[/g;
      return reg.test(line) && !(line[line.length - 1] === "]");
    }

    function arrayEnd(line: string): boolean {
      return line[line.length - 1] === "]";
    }

    function stringStart(line: string): boolean {
      const m = line.match(/.*=\s*(?:\"\"\"|''')/);
      if (!m) {
        return false;
      }
      return !line.endsWith(`"""`) || !line.endsWith(`'''`);
    }

    function stringEnd(line: string): boolean {
      return line.endsWith(`'''`) || line.endsWith(`"""`);
    }

    function isLiteralString(line: string): boolean {
      return line.match(/'''/) ? true : false;
    }

    const merged = [];
    let acc = [],
      isLiteral = false,
      capture = false,
      captureType = "",
      merge = false;

    for (let i = 0; i < this.tomlLines.length; i++) {
      const line = this.tomlLines[i];
      const trimmed = line.trim();
      if (!capture && arrayStart(trimmed)) {
        capture = true;
        captureType = "array";
      } else if (!capture && stringStart(trimmed)) {
        isLiteral = isLiteralString(trimmed);
        capture = true;
        captureType = "string";
      } else if (capture && arrayEnd(trimmed)) {
        merge = true;
      } else if (capture && stringEnd(trimmed)) {
        merge = true;
      }

      if (capture) {
        if (isLiteral) {
          acc.push(line);
        } else {
          acc.push(trimmed);
        }
      } else {
        if (isLiteral) {
          merged.push(line);
        } else {
          merged.push(trimmed);
        }
      }

      if (merge) {
        capture = false;
        merge = false;
        if (captureType === "string") {
          merged.push(
            acc
              .join("\n")
              .replace(/"""/g, '"')
              .replace(/'''/g, `'`)
              .replace(/\n/g, "\\n"),
          );
          isLiteral = false;
        } else {
          merged.push(acc.join(""));
        }
        captureType = "";
        acc = [];
      }
    }
    this.tomlLines = merged;
  }
  _unflat(
    keys: string[],
    values: Record<string, unknown> | unknown[] = {},
    cObj: Record<string, unknown> | unknown[] = {},
  ): Record<string, unknown> {
    const out: Record<string, unknown> = {};
    if (keys.length === 0) {
      return cObj as Record<string, unknown>;
    } else {
      if (Object.keys(cObj).length === 0) {
        cObj = values;
      }
      const key: string | undefined = keys.pop();
      if (key) {
        out[key] = cObj;
      }
      return this._unflat(keys, values, out);
    }
  }
  _groupToOutput(): void {
    assert(this.context.currentGroup != null, "currentGroup must be set");
    const arrProperty = this.context.currentGroup.name
      .replace(/"/g, "")
      .replace(/'/g, "")
      .split(".");
    let u = {};
    if (this.context.currentGroup.type === "array") {
      u = this._unflat(arrProperty, this.context.currentGroup.arrValues);
    } else {
      u = this._unflat(arrProperty, this.context.currentGroup.objValues);
    }
    deepAssign(this.context.output, u);
    delete this.context.currentGroup;
  }
  _split(str: string): string[] {
    const out = [];
    out.push(...str.split("\n"));
    return out;
  }
  _isGroup(line: string): boolean {
    const t = line.trim();
    return t[0] === "[" && /\[(.*)\]/.exec(t) ? true : false;
  }
  _isDeclaration(line: string): boolean {
    return line.split("=").length > 1;
  }
  _createGroup(line: string): void {
    const captureReg = /\[(.*)\]/;
    if (this.context.currentGroup) {
      this._groupToOutput();
    }

    let type;
    let m = line.match(captureReg);
    assert(m != null, "line mut be matched");
    let name = m[1];
    if (name.match(/\[.*\]/)) {
      type = "array";
      m = name.match(captureReg);
      assert(m != null, "name must be matched");
      name = m[1];
    } else {
      type = "object";
    }
    this.context.currentGroup = new ParserGroup(type, name);
  }
  _processDeclaration(line: string): KeyValuePair {
    const idx = line.indexOf("=");
    const key = line.substring(0, idx).trim();
    const value = this._parseData(line.slice(idx + 1));
    return new KeyValuePair(key, value);
  }
  _parseData(dataString: string): unknown {
    dataString = dataString.trim();
    switch (dataString[0]) {
      case '"':
      case "'":
        return this._parseString(dataString);
      case "[":
      case "{":
        return this._parseInlineTableOrArray(dataString);
      default: {
        // Strip a comment.
        const match = /#.*$/.exec(dataString);
        if (match) {
          dataString = dataString.slice(0, match.index).trim();
        }

        switch (dataString) {
          case "true":
            return true;
          case "false":
            return false;
          case "inf":
          case "+inf":
            return Infinity;
          case "-inf":
            return -Infinity;
          case "nan":
          case "+nan":
          case "-nan":
            return NaN;
          default:
            return this._parseNumberOrDate(dataString);
        }
      }
    }
  }
  _parseInlineTableOrArray(dataString: string): unknown {
    const invalidArr = /,\]/g.exec(dataString);
    if (invalidArr) {
      dataString = dataString.replace(/,]/g, "]");
    }

    if (
      (dataString[0] === "{" && dataString[dataString.length - 1] === "}") ||
      (dataString[0] === "[" && dataString[dataString.length - 1] === "]")
    ) {
      const reg = /([a-zA-Z0-9-_\.]*) (=)/gi;
      let result;
      while ((result = reg.exec(dataString))) {
        const ogVal = result[0];
        const newVal = ogVal
          .replace(result[1], `"${result[1]}"`)
          .replace(result[2], ":");
        dataString = dataString.replace(ogVal, newVal);
      }
      return JSON.parse(dataString);
    }
    throw new TOMLError("Malformed inline table or array literal");
  }
  _parseString(dataString: string): string {
    const quote = dataString[0];
    // Handle First and last EOL for multiline strings
    if (dataString.startsWith(`"\\n`)) {
      dataString = dataString.replace(`"\\n`, `"`);
    } else if (dataString.startsWith(`'\\n`)) {
      dataString = dataString.replace(`'\\n`, `'`);
    }
    if (dataString.endsWith(`\\n"`)) {
      dataString = dataString.replace(`\\n"`, `"`);
    } else if (dataString.endsWith(`\\n'`)) {
      dataString = dataString.replace(`\\n'`, `'`);
    }
    let value = "";
    for (let i = 1; i < dataString.length; i++) {
      switch (dataString[i]) {
        case "\\":
          i++;
          // See https://toml.io/en/v1.0.0-rc.3#string
          switch (dataString[i]) {
            case "b":
              value += "\b";
              break;
            case "t":
              value += "\t";
              break;
            case "n":
              value += "\n";
              break;
            case "f":
              value += "\f";
              break;
            case "r":
              value += "\r";
              break;
            case "u":
            case "U": {
              // Unicode character
              const codePointLen = dataString[i] === "u" ? 4 : 6;
              const codePoint = parseInt(
                "0x" + dataString.slice(i + 1, i + 1 + codePointLen),
                16,
              );
              value += String.fromCodePoint(codePoint);
              i += codePointLen;
              break;
            }
            case "\\":
              value += "\\";
              break;
            default:
              value += dataString[i];
              break;
          }
          break;
        case quote:
          if (dataString[i - 1] !== "\\") {
            return value;
          }
          break;
        default:
          value += dataString[i];
          break;
      }
    }
    throw new TOMLError("Incomplete string literal");
  }
  _parseNumberOrDate(dataString: string): unknown {
    if (this._isDate(dataString)) {
      return new Date(dataString);
    }

    if (this._isLocalTime(dataString)) {
      return dataString;
    }

    // If binary / octal / hex
    const hex = /^(0(?:x|o|b)[0-9a-f_]*)/gi.exec(dataString);
    if (hex && hex[0]) {
      return hex[0].trim();
    }

    const testNumber = this._isParsableNumber(dataString);
    if (testNumber !== false && !isNaN(testNumber as number)) {
      return testNumber;
    }

    return String(dataString);
  }
  _isLocalTime(str: string): boolean {
    const reg = /(\d{2}):(\d{2}):(\d{2})/;
    return reg.test(str);
  }
  _isParsableNumber(dataString: string): number | boolean {
    const m = /((?:\+|-|)[0-9_\.e+\-]*)[^#]/i.exec(dataString);
    if (!m) {
      return false;
    } else {
      return parseFloat(m[0].replace(/_/g, ""));
    }
  }
  _isDate(dateStr: string): boolean {
    const reg = /\d{4}-\d{2}-\d{2}/;
    return reg.test(dateStr);
  }
  _parseDeclarationName(declaration: string): string[] {
    const out = [];
    let acc = [];
    let inLiteral = false;
    for (let i = 0; i < declaration.length; i++) {
      const c = declaration[i];
      switch (c) {
        case ".":
          if (!inLiteral) {
            out.push(acc.join(""));
            acc = [];
          } else {
            acc.push(c);
          }
          break;
        case `"`:
          if (inLiteral) {
            inLiteral = false;
          } else {
            inLiteral = true;
          }
          break;
        default:
          acc.push(c);
          break;
      }
    }
    if (acc.length !== 0) {
      out.push(acc.join(""));
    }
    return out;
  }
  _parseLines(): void {
    for (let i = 0; i < this.tomlLines.length; i++) {
      const line = this.tomlLines[i];

      // TODO (zekth) Handle unflat of array of tables
      if (this._isGroup(line)) {
        // if the current group is an array we push the
        // parsed objects in it.
        if (
          this.context.currentGroup &&
          this.context.currentGroup.type === "array"
        ) {
          this.context.currentGroup.arrValues.push(
            this.context.currentGroup.objValues,
          );
          this.context.currentGroup.objValues = {};
        }
        // If we need to create a group or to change group
        if (
          !this.context.currentGroup ||
          (this.context.currentGroup &&
            this.context.currentGroup.name !==
              line.replace(/\[/g, "").replace(/\]/g, ""))
        ) {
          this._createGroup(line);
          continue;
        }
      }
      if (this._isDeclaration(line)) {
        const kv = this._processDeclaration(line);
        const key = kv.key;
        const value = kv.value;
        if (!this.context.currentGroup) {
          this.context.output[key] = value;
        } else {
          this.context.currentGroup.objValues[key] = value;
        }
      }
    }
    if (this.context.currentGroup) {
      if (this.context.currentGroup.type === "array") {
        this.context.currentGroup.arrValues.push(
          this.context.currentGroup.objValues,
        );
      }
      this._groupToOutput();
    }
  }
  _cleanOutput(): void {
    this._propertyClean(this.context.output);
  }
  _propertyClean(obj: Record<string, unknown>): void {
    const keys = Object.keys(obj);
    for (let i = 0; i < keys.length; i++) {
      let k = keys[i];
      if (k) {
        let v = obj[k];
        const pathDeclaration = this._parseDeclarationName(k);
        delete obj[k];
        if (pathDeclaration.length > 1) {
          const shift = pathDeclaration.shift();
          if (shift) {
            k = shift.replace(/"/g, "");
            v = this._unflat(pathDeclaration, v as Record<string, unknown>);
          }
        } else {
          k = k.replace(/"/g, "");
        }
        obj[k] = v;
        if (v instanceof Object) {
          // deno-lint-ignore no-explicit-any
          this._propertyClean(v as any);
        }
      }
    }
  }
  parse(): Record<string, unknown> {
    this._sanitize();
    this._parseLines();
    this._cleanOutput();
    return this.context.output;
  }
}

// Bare keys may only contain ASCII letters,
// ASCII digits, underscores, and dashes (A-Za-z0-9_-).
function joinKeys(keys: string[]): string {
  // Dotted keys are a sequence of bare or quoted keys joined with a dot.
  // This allows for grouping similar properties together:
  return keys
    .map((str: string): string => {
      return str.match(/[^A-Za-z0-9_-]/) ? `"${str}"` : str;
    })
    .join(".");
}

class Dumper {
  maxPad = 0;
  srcObject: Record<string, unknown>;
  output: string[] = [];
  constructor(srcObjc: Record<string, unknown>) {
    this.srcObject = srcObjc;
  }
  dump(): string[] {
    // deno-lint-ignore no-explicit-any
    this.output = this._parse(this.srcObject as any);
    this.output = this._format();
    return this.output;
  }
  _parse(obj: Record<string, unknown>, keys: string[] = []): string[] {
    const out = [];
    const props = Object.keys(obj);
    const propObj = props.filter((e: string): boolean => {
      if (obj[e] instanceof Array) {
        const d: unknown[] = obj[e] as unknown[];
        return !this._isSimplySerializable(d[0]);
      }
      return !this._isSimplySerializable(obj[e]);
    });
    const propPrim = props.filter((e: string): boolean => {
      if (obj[e] instanceof Array) {
        const d: unknown[] = obj[e] as unknown[];
        return this._isSimplySerializable(d[0]);
      }
      return this._isSimplySerializable(obj[e]);
    });
    const k = propPrim.concat(propObj);
    for (let i = 0; i < k.length; i++) {
      const prop = k[i];
      const value = obj[prop];
      if (value instanceof Date) {
        out.push(this._dateDeclaration([prop], value));
      } else if (typeof value === "string" || value instanceof RegExp) {
        out.push(this._strDeclaration([prop], value.toString()));
      } else if (typeof value === "number") {
        out.push(this._numberDeclaration([prop], value));
      } else if (typeof value === "boolean") {
        out.push(this._boolDeclaration([prop], value));
      } else if (
        value instanceof Array &&
        this._isSimplySerializable(value[0])
      ) {
        // only if primitives types in the array
        out.push(this._arrayDeclaration([prop], value));
      } else if (
        value instanceof Array &&
        !this._isSimplySerializable(value[0])
      ) {
        // array of objects
        for (let i = 0; i < value.length; i++) {
          out.push("");
          out.push(this._headerGroup([...keys, prop]));
          out.push(...this._parse(value[i], [...keys, prop]));
        }
      } else if (typeof value === "object") {
        out.push("");
        out.push(this._header([...keys, prop]));
        if (value) {
          const toParse = value as Record<string, unknown>;
          out.push(...this._parse(toParse, [...keys, prop]));
        }
        // out.push(...this._parse(value, `${path}${prop}.`));
      }
    }
    out.push("");
    return out;
  }
  _isSimplySerializable(value: unknown): boolean {
    return (
      typeof value === "string" ||
      typeof value === "number" ||
      typeof value === "boolean" ||
      value instanceof RegExp ||
      value instanceof Date ||
      value instanceof Array
    );
  }
  _header(keys: string[]): string {
    return `[${joinKeys(keys)}]`;
  }
  _headerGroup(keys: string[]): string {
    return `[[${joinKeys(keys)}]]`;
  }
  _declaration(keys: string[]): string {
    const title = joinKeys(keys);
    if (title.length > this.maxPad) {
      this.maxPad = title.length;
    }
    return `${title} = `;
  }
  _arrayDeclaration(keys: string[], value: unknown[]): string {
    return `${this._declaration(keys)}${JSON.stringify(value)}`;
  }
  _strDeclaration(keys: string[], value: string): string {
    return `${this._declaration(keys)}"${value}"`;
  }
  _numberDeclaration(keys: string[], value: number): string {
    switch (value) {
      case Infinity:
        return `${this._declaration(keys)}inf`;
      case -Infinity:
        return `${this._declaration(keys)}-inf`;
      default:
        return `${this._declaration(keys)}${value}`;
    }
  }
  _boolDeclaration(keys: string[], value: boolean): string {
    return `${this._declaration(keys)}${value}`;
  }
  _dateDeclaration(keys: string[], value: Date): string {
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
    return `${this._declaration(keys)}${fData}`;
  }
  _format(): string[] {
    const rDeclaration = /(.*)\s=/;
    const out = [];
    for (let i = 0; i < this.output.length; i++) {
      const l = this.output[i];
      // we keep empty entry for array of objects
      if (l[0] === "[" && l[1] !== "[") {
        // empty object
        if (this.output[i + 1] === "") {
          i += 1;
          continue;
        }
        out.push(l);
      } else {
        const m = rDeclaration.exec(l);
        if (m) {
          out.push(l.replace(m[1], m[1].padEnd(this.maxPad)));
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
 */
export function stringify(srcObj: Record<string, unknown>): string {
  return new Dumper(srcObj).dump().join("\n");
}

/**
 * Parse parses TOML string into an object.
 * @param tomlString
 */
export function parse(tomlString: string): Record<string, unknown> {
  // File is potentially using EOL CRLF
  tomlString = tomlString.replace(/\r\n/g, "\n").replace(/\\\n/g, "\n");
  return new Parser(tomlString).parse();
}
