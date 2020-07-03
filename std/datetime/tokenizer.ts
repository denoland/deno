export type Token = {
  type: string;
  value: string | number;
  input: string;
  index: number;
};

interface ReceiverResult {
  [name: string]: string | number;
}
export type CallbackResult = { type: string; value: string | number };
type CallbackFunction = (match: RegExpExecArray) => CallbackResult;

export interface Rule {
  test: RegExp;
  fn: CallbackFunction;
}

export class Tokenizer {
  rules: Rule[];

  constructor(rules: Rule[] = []) {
    this.rules = rules;
  }

  addRule(test: RegExp, fn: CallbackFunction): Tokenizer {
    this.rules.push({ test, fn });
    return this;
  }

  tokenize(
    string: string,
    receiver = (token: Token): { [name: string]: string | number } => token
  ): ReceiverResult[] {
    let index = 0;

    const next = (): ReceiverResult | null => {
      for (const rule of this.rules) {
        const match = rule.test.exec(string);
        if (match) {
          const value = match[0];
          index += value.length;
          string = string.slice(match[0].length);
          const token = { ...rule.fn(match), input: value, index };
          return receiver(token);
        }
      }
      return null;
    };

    const tokens = [];

    let token;
    while (!!(token = next())) tokens.push(token);

    if (string.length) {
      throw new Error(
        `parser error: string not fully parsed! ${string.slice(0, 25)}`
      );
    }

    return tokens;
  }
}
