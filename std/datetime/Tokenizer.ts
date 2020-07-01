
export type CallbackFunction = (match: any) => any

export interface Token {
  type: string
  value: any
  input: string
  index: number
}

export interface Rule {
  test: RegExp
  fn: CallbackFunction
}

export class Tokenizer {
  rules: Rule[]

  constructor(rules: Rule[] = []) {
    this.rules = rules
  }

  addRule(test: RegExp, fn: CallbackFunction) {
    this.rules.push({ test, fn })
    return this
  }

  tokenize(string: string, receiver = (token: Token): any => token): any[] {
    let index = 0

    const next = () => {
      for (const rule of this.rules) {
        const match = rule.test.exec(string)
        if (match) {
          const value = match[0]
          index += value.length
          string = string.slice(match[0].length)
          return receiver({ ...rule.fn(match), input: value, index })
        }
      }
    }

    const tokens = []
    
    for (let token; token = next();) { tokens.push(token) }

    if (string.length) { throw new Error(`parser error: string not fully parsed! ${string.slice(0, 25)}`) }

    return tokens
  }

}