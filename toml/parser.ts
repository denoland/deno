// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { existsSync } from "../fs/exists.ts";
import { deepAssign } from "../util/deep_assign.ts";

class KeyValuePair {
  key: string;
  value: unknown;
}

class ParserGroup {
  type: string;
  name: string;
  arrValues: unknown[] = [];
  objValues: object = {};
}

class ParserContext {
  currentGroup?: ParserGroup;
  output: object = {};
}

class Parser {
  tomlLines: string[];
  context: ParserContext;
  constructor(tomlString: string) {
    this.tomlLines = this._split(tomlString);
    this.context = new ParserContext();
  }
  _sanitize(): void {
    const out = [];
    for (let i = 0; i < this.tomlLines.length; i++) {
      const s = this.tomlLines[i];
      const trimmed = s.trim();
      if (trimmed !== "" && trimmed[0] !== "#") {
        out.push(s);
      }
    }
    this.tomlLines = out;
    this._mergeMultilines();
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

    let merged = [],
      acc = [],
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
              .replace(/\n/g, "\\n")
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
  _unflat(keys: string[], values: object = {}, cObj: object = {}): object {
    let out = {};
    if (keys.length === 0) {
      return cObj;
    } else {
      if (Object.keys(cObj).length === 0) {
        cObj = values;
      }
      let key = keys.pop();
      out[key] = cObj;
      return this._unflat(keys, values, out);
    }
  }
  _groupToOutput(): void {
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
    let out = [];
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
    let g = new ParserGroup();
    g.name = line.match(captureReg)[1];
    if (g.name.match(/\[.*\]/)) {
      g.type = "array";
      g.name = g.name.match(captureReg)[1];
    } else {
      g.type = "object";
    }
    this.context.currentGroup = g;
  }
  _processDeclaration(line: string): KeyValuePair {
    let kv = new KeyValuePair();
    const idx = line.indexOf("=");
    kv.key = line.substring(0, idx).trim();
    kv.value = this._parseData(line.slice(idx + 1));
    return kv;
  }
  // TODO (zekth) Need refactor using ACC
  _parseData(dataString: string): unknown {
    dataString = dataString.trim();

    if (this._isDate(dataString)) {
      return new Date(dataString.split("#")[0].trim());
    }

    if (this._isLocalTime(dataString)) {
      return eval(`"${dataString.split("#")[0].trim()}"`);
    }

    const cut3 = dataString.substring(0, 3).toLowerCase();
    const cut4 = dataString.substring(0, 4).toLowerCase();
    if (cut3 === "inf" || cut4 === "+inf") {
      return Infinity;
    }
    if (cut4 === "-inf") {
      return -Infinity;
    }

    if (cut3 === "nan" || cut4 === "+nan" || cut4 === "-nan") {
      return NaN;
    }

    // If binary / octal / hex
    const hex = /(0(?:x|o|b)[0-9a-f_]*)[^#]/gi.exec(dataString);
    if (hex && hex[0]) {
      return hex[0].trim();
    }

    const testNumber = this._isParsableNumber(dataString);
    if (testNumber && !isNaN(testNumber as number)) {
      return testNumber;
    }

    const invalidArr = /,\]/g.exec(dataString);
    if (invalidArr) {
      dataString = dataString.replace(/,]/g, "]");
    }
    const m = /(?:\'|\[|{|\").*(?:\'|\]|\"|})\s*[^#]/g.exec(dataString);
    if (m) {
      dataString = m[0].trim();
    }
    if (dataString[0] === "{" && dataString[dataString.length - 1] === "}") {
      const reg = /([a-zA-Z0-9-_\.]*) (=)/gi;
      let result;
      while ((result = reg.exec(dataString))) {
        let ogVal = result[0];
        let newVal = ogVal
          .replace(result[1], `"${result[1]}"`)
          .replace(result[2], ":");
        dataString = dataString.replace(ogVal, newVal);
      }
      return JSON.parse(dataString);
    }

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
    return eval(dataString);
  }
  _isLocalTime(str: string): boolean {
    const reg = /(\d{2}):(\d{2}):(\d{2})/;
    return reg.test(str);
  }
  _isParsableNumber(dataString: string): number | boolean {
    const m = /((?:\+|-|)[0-9_\.e+\-]*)[^#]/i.exec(dataString.trim());
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
    let inLitteral = false;
    for (let i = 0; i < declaration.length; i++) {
      const c = declaration[i];
      switch (c) {
        case ".":
          if (!inLitteral) {
            out.push(acc.join(""));
            acc = [];
          } else {
            acc.push(c);
          }
          break;
        case `"`:
          if (inLitteral) {
            inLitteral = false;
          } else {
            inLitteral = true;
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
            this.context.currentGroup.objValues
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
        let kv = this._processDeclaration(line);
        let key = kv.key;
        let value = kv.value;
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
          this.context.currentGroup.objValues
        );
      }
      this._groupToOutput();
    }
  }
  _cleanOutput(): void {
    this._propertyClean(this.context.output);
  }
  _propertyClean(obj: object): void {
    const keys = Object.keys(obj);
    for (let i = 0; i < keys.length; i++) {
      let k = keys[i];
      let v = obj[k];
      let pathDeclaration = this._parseDeclarationName(k);
      delete obj[k];
      if (pathDeclaration.length > 1) {
        k = pathDeclaration.shift();
        k = k.replace(/"/g, "");
        v = this._unflat(pathDeclaration, v as object);
      } else {
        k = k.replace(/"/g, "");
      }
      obj[k] = v;
      if (v instanceof Object) {
        this._propertyClean(v);
      }
    }
  }
  parse(): object {
    this._sanitize();
    this._parseLines();
    this._cleanOutput();
    return this.context.output;
  }
}

export function parse(tomlString: string): object {
  // File is potentially using EOL CRLF
  tomlString = tomlString.replace(/\r\n/g, "\n").replace(/\\\n/g, "\n");
  return new Parser(tomlString).parse();
}

export function parseFile(filePath: string): object {
  if (!existsSync(filePath)) {
    throw new Error("File not found");
  }
  const decoder = new TextDecoder();
  const strFile = decoder.decode(Deno.readFileSync(filePath));
  return parse(strFile);
}
