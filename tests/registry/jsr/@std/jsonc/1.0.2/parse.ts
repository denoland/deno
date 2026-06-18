// Copyright 2018-2025 the Deno authors. MIT license.
// This module is browser compatible.

import type { JsonValue } from "jsr:@std/json@^1.0.2/types";
export type { JsonValue };

/**
 * Converts a JSON with Comments (JSONC) string into an object.
 *
 * @example Usage
 * ```ts
 * import { parse } from "@std/jsonc";
 * import { assertEquals } from "@std/assert";
 *
 * assertEquals(parse('{"foo": "bar"}'), { foo: "bar" });
 * assertEquals(parse('{"foo": "bar", }'), { foo: "bar" });
 * assertEquals(parse('{"foo": "bar", } /* comment *\/'), { foo: "bar" });
 * ```
 *
 * @throws {SyntaxError} If the JSONC string is invalid.
 * @param text A valid JSONC string.
 * @returns The parsed JsonValue from the JSONC string.
 */
export function parse(text: string): JsonValue {
  if (new.target) {
    throw new TypeError(
      "Cannot create an instance: parse is not a constructor",
    );
  }
  return new JsoncParser(text).parse();
}

type TokenType =
  | "BeginObject"
  | "EndObject"
  | "BeginArray"
  | "EndArray"
  | "NameSeparator"
  | "ValueSeparator"
  | "NullOrTrueOrFalseOrNumber"
  | "String";

type Token = {
  type: Exclude<
    TokenType,
    "String" | "NullOrTrueOrFalseOrNumber"
  >;
  sourceText?: undefined;
  position: number;
} | {
  type: "String";
  sourceText: string;
  position: number;
} | {
  type: "NullOrTrueOrFalseOrNumber";
  sourceText: string;
  position: number;
};

// First tokenize and then parse the token.
class JsoncParser {
  readonly #whitespace = new Set(" \t\r\n");
  readonly #numberEndToken = new Set([..."[]{}:,/", ...this.#whitespace]);
  #text: string;
  #length: number;
  #tokenized: Generator<Token, void>;
  constructor(text: string) {
    this.#text = `${text}`;
    this.#length = this.#text.length;
    this.#tokenized = this.#tokenize();
  }
  parse(): JsonValue {
    const token = this.#getNext();
    const res = this.#parseJsonValue(token);

    // make sure all characters have been read
    const { done, value } = this.#tokenized.next();
    if (!done) {
      throw new SyntaxError(buildErrorMessage(value));
    }

    return res;
  }
  /** Read the next token. If the token is read to the end, it throws a SyntaxError. */
  #getNext(): Token {
    const { done, value } = this.#tokenized.next();
    if (done) {
      throw new SyntaxError(
        "Cannot parse JSONC: unexpected end of JSONC input",
      );
    }
    return value;
  }
  /** Split the JSONC string into token units. Whitespace and comments are skipped. */
  *#tokenize(): Generator<Token, void> {
    for (let i = 0; i < this.#length; i++) {
      // skip whitespace
      if (this.#whitespace.has(this.#text[i]!)) {
        continue;
      }

      // skip multi line comment (`/*...*/`)
      if (this.#text[i] === "/" && this.#text[i + 1] === "*") {
        i += 2;
        let hasEndOfComment = false;
        for (; i < this.#length; i++) { // read until find `*/`
          if (this.#text[i] === "*" && this.#text[i + 1] === "/") {
            hasEndOfComment = true;
            break;
          }
        }
        if (!hasEndOfComment) {
          throw new SyntaxError(
            "Cannot parse JSONC: unexpected end of JSONC input",
          );
        }
        i++;
        continue;
      }

      // skip single line comment (`//...`)
      if (this.#text[i] === "/" && this.#text[i + 1] === "/") {
        i += 2;
        for (; i < this.#length; i++) { // read until find `\n` or `\r`
          if (this.#text[i] === "\n" || this.#text[i] === "\r") {
            break;
          }
        }
        continue;
      }

      switch (this.#text[i]) {
        case "{":
          yield { type: "BeginObject", position: i };
          break;
        case "}":
          yield { type: "EndObject", position: i };
          break;
        case "[":
          yield { type: "BeginArray", position: i };
          break;
        case "]":
          yield { type: "EndArray", position: i };
          break;
        case ":":
          yield { type: "NameSeparator", position: i };
          break;
        case ",":
          yield { type: "ValueSeparator", position: i };
          break;
        case '"': { // parse string token
          const startIndex = i;
          // Need to handle consecutive backslashes correctly
          // '"\\""' => '"'
          // '"\\\\"' => '\\'
          // '"\\\\\\""' => '\\"'
          // '"\\\\\\\\"' => '\\\\'
          let shouldEscapeNext = false;
          i++;
          for (; i < this.#length; i++) { // read until find `"`
            if (this.#text[i] === '"' && !shouldEscapeNext) {
              break;
            }
            shouldEscapeNext = this.#text[i] === "\\" && !shouldEscapeNext;
          }
          yield {
            type: "String",
            sourceText: this.#text.substring(startIndex, i + 1),
            position: startIndex,
          };
          break;
        }
        default: { // parse null, true, false or number token
          const startIndex = i;
          for (; i < this.#length; i++) { // read until find numberEndToken
            if (this.#numberEndToken.has(this.#text[i]!)) {
              break;
            }
          }
          i--;
          yield {
            type: "NullOrTrueOrFalseOrNumber",
            sourceText: this.#text.substring(startIndex, i + 1),
            position: startIndex,
          };
        }
      }
    }
  }

  #parseJsonValue(value: Token): JsonValue {
    switch (value.type) {
      case "BeginObject":
        return this.#parseObject();
      case "BeginArray":
        return this.#parseArray();
      case "NullOrTrueOrFalseOrNumber":
        return this.#parseNullOrTrueOrFalseOrNumber(value);
      case "String":
        return this.#parseString(value);
      default:
        throw new SyntaxError(buildErrorMessage(value));
    }
  }

  #parseObject(): { [key: string]: JsonValue | undefined } {
    const target: { [key: string]: JsonValue | undefined } = {};
    //   ┌─token1
    // { }
    //      ┌─────────────token1
    //      │   ┌─────────token2
    //      │   │   ┌─────token3
    //      │   │   │   ┌─token4
    //  { "key" : value }
    //      ┌───────────────token1
    //      │   ┌───────────token2
    //      │   │   ┌───────token3
    //      │   │   │   ┌───token4
    //      │   │   │   │ ┌─token1
    //  { "key" : value , }
    //      ┌─────────────────────────────token1
    //      │   ┌─────────────────────────token2
    //      │   │   ┌─────────────────────token3
    //      │   │   │   ┌─────────────────token4
    //      │   │   │   │   ┌─────────────token1
    //      │   │   │   │   │   ┌─────────token2
    //      │   │   │   │   │   │   ┌─────token3
    //      │   │   │   │   │   │   │   ┌─token4
    //  { "key" : value , "key" : value }
    while (true) {
      const token1 = this.#getNext();
      if (token1.type === "EndObject") {
        return target;
      }
      if (token1.type !== "String") {
        throw new SyntaxError(buildErrorMessage(token1));
      }
      const key = this.#parseString(token1);

      const token2 = this.#getNext();
      if (token2.type !== "NameSeparator") {
        throw new SyntaxError(buildErrorMessage(token2));
      }

      const token3 = this.#getNext();
      Object.defineProperty(target, key, {
        value: this.#parseJsonValue(token3),
        writable: true,
        enumerable: true,
        configurable: true,
      });

      const token4 = this.#getNext();
      if (token4.type === "EndObject") {
        return target;
      }
      if (token4.type !== "ValueSeparator") {
        throw new SyntaxError(buildErrorMessage(token4));
      }
    }
  }

  #parseArray(): JsonValue[] {
    const target: JsonValue[] = [];
    //   ┌─token1
    // [ ]
    //      ┌─────────────token1
    //      │   ┌─────────token2
    //  [ value ]
    //      ┌───────token1
    //      │   ┌───token2
    //      │   │ ┌─token1
    //  [ value , ]
    //      ┌─────────────token1
    //      │   ┌─────────token2
    //      │   │   ┌─────token1
    //      │   │   │   ┌─token2
    //  [ value , value ]
    while (true) {
      const token1 = this.#getNext();
      if (token1.type === "EndArray") {
        return target;
      }
      target.push(this.#parseJsonValue(token1));

      const token2 = this.#getNext();
      if (token2.type === "EndArray") {
        return target;
      }
      if (token2.type !== "ValueSeparator") {
        throw new SyntaxError(buildErrorMessage(token2));
      }
    }
  }

  #parseString(value: {
    type: "String";
    sourceText: string;
    position: number;
  }): string {
    let parsed;
    try {
      // Use JSON.parse to handle `\u0000` etc. correctly.
      parsed = JSON.parse(value.sourceText);
    } catch {
      throw new SyntaxError(buildErrorMessage(value));
    }
    if (typeof parsed !== "string") {
      throw new TypeError(`Parsed value is not a string: ${parsed}`);
    }
    return parsed;
  }

  #parseNullOrTrueOrFalseOrNumber(value: {
    type: "NullOrTrueOrFalseOrNumber";
    sourceText: string;
    position: number;
  }): null | boolean | number {
    if (value.sourceText === "null") {
      return null;
    }
    if (value.sourceText === "true") {
      return true;
    }
    if (value.sourceText === "false") {
      return false;
    }
    let parsed;
    try {
      // Use JSON.parse to handle `+100`, `Infinity` etc. correctly.
      parsed = JSON.parse(value.sourceText);
    } catch {
      throw new SyntaxError(buildErrorMessage(value));
    }
    if (typeof parsed !== "number") {
      throw new TypeError(`Parsed value is not a number: ${parsed}`);
    }
    return parsed;
  }
}

function buildErrorMessage({ type, sourceText, position }: Token): string {
  let token = "";
  switch (type) {
    case "BeginObject":
      token = "{";
      break;
    case "EndObject":
      token = "}";
      break;
    case "BeginArray":
      token = "[";
      break;
    case "EndArray":
      token = "]";
      break;
    case "NameSeparator":
      token = ":";
      break;
    case "ValueSeparator":
      token = ",";
      break;
    case "NullOrTrueOrFalseOrNumber":
    case "String":
      // Truncate the string so that it is within 30 lengths.
      token = 30 < sourceText.length
        ? `${sourceText.slice(0, 30)}...`
        : sourceText;
      break;
  }
  return `Cannot parse JSONC: unexpected token "${token}" in JSONC at position ${position}`;
}
