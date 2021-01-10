// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

export type Token = {
  type: string;
  value: string | number;
  index: number;
  [key: string]: unknown;
};

export interface ReceiverResult {
  [name: string]: string | number | unknown;
}
export type CallbackResult = {
  type: string;
  value: string | number;
  [key: string]: unknown;
};
type CallbackFunction = (value: unknown) => CallbackResult;

export type TestResult = { value: unknown; length: number } | undefined;
export type TestFunction = (
  string: string,
) => TestResult | undefined;

export interface Rule {
  test: TestFunction;
  fn: CallbackFunction;
}

export class Tokenizer {
  rules: Rule[];

  constructor(rules: Rule[] = []) {
    this.rules = rules;
  }

  addRule(test: TestFunction, fn: CallbackFunction): Tokenizer {
    this.rules.push({ test, fn });
    return this;
  }

  tokenize(
    string: string,
    receiver = (token: Token): ReceiverResult => token,
  ): ReceiverResult[] {
    function* generator(rules: Rule[]): IterableIterator<ReceiverResult> {
      let index = 0;
      for (const rule of rules) {
        const result = rule.test(string);
        if (result) {
          const { value, length } = result;
          index += length;
          string = string.slice(length);
          const token = { ...rule.fn(value), index };
          yield receiver(token);
          yield* generator(rules);
        }
      }
    }
    const tokenGenerator = generator(this.rules);

    const tokens: ReceiverResult[] = [];

    for (const token of tokenGenerator) {
      tokens.push(token);
    }

    if (string.length) {
      throw new Error(
        `parser error: string not fully parsed! ${string.slice(0, 25)}`,
      );
    }

    return tokens;
  }
}
