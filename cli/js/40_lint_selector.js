// Copyright 2018-2025 the Deno authors. MIT license.

// @ts-check

/** @typedef {import("./40_lint_types.d.ts").LintState} LintState */
/** @typedef {import("./40_lint_types.d.ts").AstContext} AstContext */
/** @typedef {import("./40_lint_types.d.ts").MatchContext} MatchCtx */
/** @typedef {import("./40_lint_types.d.ts").AttrExists} AttrExists */
/** @typedef {import("./40_lint_types.d.ts").AttrBin} AttrBin */
/** @typedef {import("./40_lint_types.d.ts").AttrSelector} AttrSelector */
/** @typedef {import("./40_lint_types.d.ts").ElemSelector} ElemSelector */
/** @typedef {import("./40_lint_types.d.ts").FieldSelector} FieldSelector */
/** @typedef {import("./40_lint_types.d.ts").PseudoNthChild} PseudoNthChild */
/** @typedef {import("./40_lint_types.d.ts").PseudoHas} PseudoHas */
/** @typedef {import("./40_lint_types.d.ts").PseudoNot} PseudoNot */
/** @typedef {import("./40_lint_types.d.ts").Relation} SRelation */
/** @typedef {import("./40_lint_types.d.ts").Selector} Selector */
/** @typedef {import("./40_lint_types.d.ts").SelectorParseCtx} SelectorParseCtx */
/** @typedef {import("./40_lint_types.d.ts").MatcherFn} MatcherFn */
/** @typedef {import("./40_lint_types.d.ts").TransformFn} Transformer */

const Char = {
  Tab: 9,
  Space: 32,
  Bang: 33,
  DoubleQuote: 34,
  Quote: 39,
  BraceOpen: 40,
  BraceClose: 41,
  Plus: 43,
  Comma: 44,
  Minus: 45,
  Dot: 46,
  Slash: 47,
  n0: 49,
  n9: 57,
  Colon: 58,
  Less: 60,
  Equal: 61,
  Greater: 62,
  A: 65,
  Z: 90,
  BracketOpen: 91,
  BackSlash: 92,
  BracketClose: 93,
  Underscore: 95,
  a: 97,
  z: 122,
  Tilde: 126,
};

export const Token = {
  EOF: 0,
  Word: 1,
  Space: 2,
  Op: 3,
  Colon: 4,
  Comma: 7,
  BraceOpen: 8,
  BraceClose: 9,
  BracketOpen: 10,
  BracketClose: 11,
  String: 12,
  Number: 13,
  Bool: 14,
  Null: 15,
  Undefined: 16,
  Dot: 17,
  Minus: 17,
};

export const BinOp = {
  /** [attr="value"] or [attr=value] */
  Equal: 1,
  /** [attr!="value"] or [attr!=value] */
  NotEqual: 2,
  /** [attr>1] */
  Greater: 3,
  /** [attr>=1] */
  GreaterThan: 4,
  /** [attr<1] */
  Less: 5,
  /** [attr<=1] */
  LessThan: 6,
  Tilde: 7,
  Plus: 8,
  Space: 9,
};

/**
 * @param {string} s
 * @returns {number}
 */
function getAttrOp(s) {
  switch (s) {
    case "=":
      return BinOp.Equal;
    case "!=":
      return BinOp.NotEqual;
    case ">":
      return BinOp.Greater;
    case ">=":
      return BinOp.GreaterThan;
    case "<":
      return BinOp.Less;
    case "<=":
      return BinOp.LessThan;
    case "~":
      return BinOp.Tilde;
    case "+":
      return BinOp.Plus;
    default:
      throw new Error(`Unknown attribute operator: '${s}'`);
  }
}

export class Lexer {
  token = Token.Word;
  start = 0;
  end = 0;
  ch = 0;
  i = -1;

  value = "";

  /**
   * @param {string} input
   */
  constructor(input) {
    this.input = input;
    this.step();
    this.next();
  }

  /**
   * @param {number} token
   */
  expect(token) {
    if (this.token !== token) {
      throw new Error(
        `Expected token '${token}', but got '${this.token}'.\n\n${this.input}\n${
          " ".repeat(this.i)
        }^`,
      );
    }
  }

  /**
   * @param {number} token
   */
  readAsWordUntil(token) {
    const s = this.i;
    while (this.token !== Token.EOF && this.token !== token) {
      this.next();
    }

    this.start = s;
    this.end = this.i - 1;
    this.value = this.getSlice();
  }

  getSlice() {
    return this.input.slice(this.start, this.end);
  }

  step() {
    this.i++;
    if (this.i >= this.input.length) {
      this.ch = -1;
    } else {
      this.ch = this.input.charCodeAt(this.i);
    }
  }

  peek() {
    const value = this.value;
    const start = this.start;
    const end = this.end;
    const i = this.i;
    const ch = this.ch;
    const token = this.token;

    this.next();

    const result = {
      token: this.token,
      value: this.value,
    };

    this.vaue = value;
    this.start = start;
    this.end = end;
    this.i = i;
    this.ch = ch;
    this.token = token;

    return result;
  }

  next() {
    this.value = "";

    if (this.i >= this.input.length) {
      this.token = Token.EOF;
      return;
    }

    // console.log(
    //   "NEXT",
    //   this.input,
    //   this.i,
    //   JSON.stringify(String.fromCharCode(this.ch)),
    // );

    while (true) {
      switch (this.ch) {
        case Char.Space:
          while (this.isWhiteSpace()) {
            this.step();
          }

          // Check if space preceeded operator
          if (this.isOpContinue()) {
            continue;
          }

          this.token = Token.Space;
          return;
        case Char.BracketOpen:
          this.token = Token.BracketOpen;
          this.step();
          return;
        case Char.BracketClose:
          this.token = Token.BracketClose;
          this.step();
          return;
        case Char.BraceOpen:
          this.token = Token.BraceOpen;
          this.step();
          return;
        case Char.BraceClose:
          this.token = Token.BraceClose;
          this.step();
          return;
        case Char.Colon:
          this.token = Token.Colon;
          this.step();
          return;
        case Char.Comma:
          this.token = Token.Comma;
          this.step();
          return;
        case Char.Dot:
          this.token = Token.Dot;
          this.step();
          return;
        case Char.Minus:
          this.token = Token.Minus;
          this.step();
          return;

        case Char.Plus:
        case Char.Tilde:
        case Char.Greater:
        case Char.Equal:
        case Char.Less:
        case Char.Bang: {
          this.token = Token.Op;
          this.start = this.i;
          this.step();

          while (this.isOpContinue()) {
            this.step();
          }

          this.end = this.i;
          this.value = this.getSlice();

          // Consume remaining space
          while (this.isWhiteSpace()) {
            this.step();
          }

          return;
        }

        case Char.Quote:
        case Char.DoubleQuote: {
          this.token = Token.String;
          const ch = this.ch;

          this.step();
          this.start = this.i;

          while (this.ch > 0 && this.ch !== ch) {
            this.step();
          }

          this.end = this.i;
          this.value = this.getSlice();
          this.step();

          return;
        }

        default:
          this.start = this.i;
          this.step();

          while (this.isWordContinue()) {
            this.step();
          }

          this.end = this.i;
          this.value = this.getSlice();
          this.token = Token.Word;
          return;
      }
    }
  }

  isWordContinue() {
    const ch = this.ch;
    switch (ch) {
      case Char.Minus:
      case Char.Underscore:
        return true;
      default:
        return (ch >= Char.a && ch <= Char.z) ||
          (ch >= Char.A && ch <= Char.Z) ||
          (ch >= Char.n0 && ch <= Char.n9);
    }
  }

  isOpContinue() {
    const ch = this.ch;
    switch (ch) {
      case Char.Equal:
      case Char.Bang:
      case Char.Greater:
      case Char.Less:
      case Char.Tilde:
      case Char.Plus:
        return true;
      default:
        return false;
    }
  }

  isWhiteSpace() {
    return this.ch === Char.Space || this.ch === Char.Tab;
  }
}

const NUMBER_REG = /^(\d+\.)?\d+$/;
const BIGINT_REG = /^\d+n$/;

/**
 * @param {string} raw
 * @returns {any}
 */
function getFromRawValue(raw) {
  switch (raw) {
    case "true":
      return true;
    case "false":
      return false;
    case "null":
      return null;
    case "undefined":
      return undefined;
    default:
      if (raw.startsWith("'") && raw.endsWith("'")) {
        if (raw.length === 2) return "";
        return raw.slice(1, -1);
      } else if (raw.startsWith('"') && raw.endsWith('"')) {
        if (raw.length === 2) return "";
        return raw.slice(1, -1);
      } else if (raw.startsWith("/")) {
        const end = raw.lastIndexOf("/");
        if (end === -1) throw new Error(`Invalid RegExp pattern: ${raw}`);
        const pattern = raw.slice(1, end);
        const flags = end < raw.length - 1 ? raw.slice(end + 1) : undefined;
        return new RegExp(pattern, flags);
      } else if (NUMBER_REG.test(raw)) {
        return Number(raw);
      } else if (BIGINT_REG.test(raw)) {
        return BigInt(raw.slice(0, -1));
      }

      return raw;
  }
}

export const ELEM_NODE = 1;
export const RELATION_NODE = 2;
export const ATTR_EXISTS_NODE = 3;
export const ATTR_BIN_NODE = 4;
export const PSEUDO_NTH_CHILD = 5;
export const PSEUDO_HAS = 6;
export const PSEUDO_NOT = 7;
export const PSEUDO_FIRST_CHILD = 8;
export const PSEUDO_LAST_CHILD = 9;
export const FIELD_NODE = 10;
export const PSEUDO_IS = 11;

/**
 * Parse out all unique selectors of a selector list.
 * @param {string} input
 * @returns {string[]}
 */
export function splitSelectors(input) {
  /** @type {string[]} */
  const out = [];

  let last = 0;
  let depth = 0;
  for (let i = 0; i < input.length; i++) {
    const ch = input.charCodeAt(i);
    switch (ch) {
      case Char.BraceOpen:
        depth++;
        break;
      case Char.BraceClose:
        depth--;
        break;
      case Char.Comma:
        if (depth === 0) {
          out.push(input.slice(last, i).trim());
          last = i + 1;
        }
        break;
    }
  }

  const remaining = input.slice(last).trim();
  if (remaining.length > 0) {
    out.push(remaining);
  }

  return out;
}

/**
 * @param {string} input
 * @param {Transformer} toElem
 * @param {Transformer} toAttr
 * @returns {Selector[]}
 */
export function parseSelector(input, toElem, toAttr) {
  /** @type {Selector[]} */
  const result = [];

  /** @type {Selector[]} */
  const stack = [[]];

  const lex = new Lexer(input);

  // Some subselectors like `:nth-child(.. of <selector>)` must have
  // a single selector instead of selector list.
  let throwOnComma = false;

  while (lex.token !== Token.EOF) {
    const current = /** @type {Selector} */ (stack.at(-1));

    if (lex.token === Token.Word) {
      const value = lex.value;
      const wildcard = value === "*";

      const elem = !wildcard ? toElem(value) : 0;
      current.push({
        type: ELEM_NODE,
        elem,
        wildcard,
      });
      lex.next();

      continue;
    } else if (lex.token === Token.Space) {
      lex.next();

      if (lex.token === Token.Word) {
        current.push({
          type: RELATION_NODE,
          op: BinOp.Space,
        });
      } else if (lex.token === Token.Colon) {
        const peeked = lex.peek();

        if (
          peeked.token === Token.Word &&
          (peeked.value === "is" || peeked.value === "where" ||
            peeked.value === "matches")
        ) {
          current.push({
            type: RELATION_NODE,
            op: BinOp.Space,
          });
        }
      }

      continue;
    } else if (lex.token === Token.BracketOpen) {
      lex.next();
      lex.expect(Token.Word);

      // Check for value comparison
      const prop = [toAttr(lex.value)];
      lex.next();

      while (lex.token === Token.Dot) {
        lex.next();
        lex.expect(Token.Word);

        prop.push(toAttr(lex.value));
        lex.next();
      }

      if (lex.token === Token.Op) {
        const op = getAttrOp(lex.value);
        lex.readAsWordUntil(Token.BracketClose);

        const value = getFromRawValue(lex.value);
        current.push({ type: ATTR_BIN_NODE, prop, op, value });
      } else {
        current.push({
          type: ATTR_EXISTS_NODE,
          prop,
        });
      }

      lex.expect(Token.BracketClose);
      lex.next();
      continue;
    } else if (lex.token === Token.Dot) {
      lex.next();
      lex.expect(Token.Word);

      const props = [toAttr(lex.value)];
      lex.next();

      while (lex.token === Token.Dot) {
        lex.next();
        lex.expect(Token.Word);

        props.push(toAttr(lex.value));
        lex.next();
      }

      current.push({
        type: FIELD_NODE,
        props,
      });
      continue;
    } else if (lex.token === Token.Colon) {
      lex.next();
      lex.expect(Token.Word);

      switch (lex.value) {
        case "first-child":
          current.push({
            type: PSEUDO_FIRST_CHILD,
          });
          break;
        case "last-child":
          current.push({
            type: PSEUDO_LAST_CHILD,
          });
          break;
        case "nth-child": {
          lex.next();
          lex.expect(Token.BraceOpen);
          lex.next();

          let mul = 1;
          let repeat = false;
          let step = 0;
          if (lex.token === Token.Minus) {
            mul = -1;
            lex.next();
          }

          lex.expect(Token.Word);
          const value = lex.getSlice();

          if (value.endsWith("n")) {
            repeat = true;
            step = +value.slice(0, -1) * mul;
          } else {
            step = +value * mul;
          }

          lex.next();

          /** @type {PseudoNthChild} */
          const node = {
            type: PSEUDO_NTH_CHILD,
            of: null,
            op: null,
            step,
            stepOffset: 0,
            repeat,
          };
          current.push(node);

          if (lex.token === Token.Space) lex.next();

          if (lex.token !== Token.BraceClose) {
            if (lex.token === Token.Op) {
              node.op = lex.value;
              lex.next();

              if (lex.token === Token.Space) lex.next();
            } else if (lex.token === Token.Minus) {
              node.op = "-";
              lex.next();

              if (lex.token === Token.Space) {
                lex.next();
              }
            }

            lex.expect(Token.Word);
            node.stepOffset = +lex.value;
            lex.next();

            if (lex.token !== Token.BraceClose) {
              lex.next(); // Space

              if (lex.token === Token.Word) {
                if (/** @type {string} */ (lex.value) !== "of") {
                  throw new Error(
                    `Expected 'of' keyword in ':nth-child' but got: ${lex.value}`,
                  );
                }

                lex.next();
                lex.expect(Token.Space);
                lex.next();
                throwOnComma = true;
                stack.push([]);
              }

              continue;
            }

            lex.expect(Token.BraceClose);
          } else if (!node.repeat) {
            // :nth-child(2) -> step is actually stepOffset
            node.stepOffset = node.step - 1;
            node.step = 0;
          }

          lex.next();

          continue;
        }
        case "where":
        case "matches":
        case "is": {
          lex.next();
          lex.expect(Token.BraceOpen);
          lex.next();

          current.push({
            type: PSEUDO_IS,
            selectors: [],
          });
          stack.push([]);

          continue;
        }
        case "has": {
          lex.next();
          lex.expect(Token.BraceOpen);
          lex.next();

          current.push({
            type: PSEUDO_HAS,
            selectors: [],
          });
          stack.push([]);

          continue;
        }
        case "not": {
          lex.next();
          lex.expect(Token.BraceOpen);
          lex.next();

          current.push({
            type: PSEUDO_NOT,
            selectors: [],
          });
          stack.push([]);

          continue;
        }
        default:
          throw new Error(`Unknown pseudo selector: '${lex.value}'`);
      }
    } else if (lex.token === Token.Comma) {
      if (throwOnComma) {
        throw new Error(`Multiple selector arguments not supported here`);
      }

      lex.next();
      if (lex.token === Token.Space) {
        lex.next();
      }

      popSelector(result, stack);
      stack.push([]);
      continue;
    } else if (lex.token === Token.BraceClose) {
      throwOnComma = false;
      popSelector(result, stack);
    } else if (lex.token === Token.Op) {
      current.push({
        type: RELATION_NODE,
        op: getAttrOp(lex.value),
      });
    }

    lex.next();
  }

  if (stack.length > 0) {
    result.push(stack[0]);
  }

  return result;
}

/**
 * @param {Selector[]} result
 * @param {Selector[]} stack
 */
function popSelector(result, stack) {
  const sel = /** @type {Selector} */ (stack.pop());

  if (stack.length === 0) {
    result.push(sel);
    stack.push([]);
  } else {
    const prev = /** @type {Selector} */ (stack.at(-1));
    if (prev.length === 0) {
      throw new Error(`Empty selector`);
    }

    const node = prev.at(-1);
    if (node === undefined) {
      throw new Error(`Empty node`);
    }

    if (node.type === PSEUDO_NTH_CHILD) {
      node.of = sel;
    } else if (
      node.type === PSEUDO_HAS || node.type === PSEUDO_IS ||
      node.type === PSEUDO_NOT
    ) {
      node.selectors.push(sel);
    } else {
      throw new Error(`Multiple selectors not allowed here`);
    }
  }
}

const TRUE_FN = () => {
  return true;
};

/**
 * @param {Selector} selector
 * @returns {MatcherFn}
 */
export function compileSelector(selector) {
  /** @type {MatcherFn} */
  let fn = TRUE_FN;

  for (let i = 0; i < selector.length; i++) {
    const node = selector[i];

    switch (node.type) {
      case ELEM_NODE:
        fn = matchElem(node, fn);
        break;
      case FIELD_NODE:
        fn = matchField(node, fn);
        break;
      case RELATION_NODE:
        switch (node.op) {
          case BinOp.Space:
            fn = matchDescendant(fn);
            break;
          case BinOp.Greater:
            fn = matchChild(fn);
            break;
          case BinOp.Plus:
            fn = matchAdjacent(fn);
            break;
          case BinOp.Tilde:
            fn = matchFollowing(fn);
            break;
          default:
            throw new Error(`Unknown relation op ${node.op}`);
        }
        break;
      case ATTR_EXISTS_NODE:
        fn = matchAttrExists(node, fn);
        break;
      case ATTR_BIN_NODE:
        fn = matchAttrBin(node, fn);
        break;
      case PSEUDO_FIRST_CHILD:
        fn = matchFirstChild(fn);
        break;
      case PSEUDO_LAST_CHILD:
        fn = matchLastChild(fn);
        break;
      case PSEUDO_NTH_CHILD:
        fn = matchNthChild(node, fn);
        break;
      case PSEUDO_HAS:
        fn = matchHas(node.selectors, fn);
        break;
      case PSEUDO_IS:
        fn = matchIs(node.selectors, fn);
        break;
      case PSEUDO_NOT:
        fn = matchNot(node.selectors, fn);
        break;
      default:
        // @ts-ignore error handling
        // deno-lint-ignore no-console
        console.log(node);
        throw new Error(`Unknown selector node`);
    }
  }

  return fn;
}

/**
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchFirstChild(next) {
  return (ctx, id) => {
    const first = ctx.getFirstChild(id);
    return first === id && next(ctx, first);
  };
}

/**
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchLastChild(next) {
  return (ctx, id) => {
    const last = ctx.getLastChild(id);
    return last === id && next(ctx, id);
  };
}

/**
 * @param {PseudoNthChild} node
 * @param {number} i
 * @returns {number}
 */
function getNthAnB(node, i) {
  const n = node.step * i;

  if (node.op === null) return n;

  switch (node.op) {
    case "+":
      return n + node.stepOffset;
    case "-":
      return n - node.stepOffset;
    default:
      throw new Error("Not supported nth-child operator: " + node.op);
  }
}

/**
 * @param {PseudoNthChild} node
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchNthChild(node, next) {
  const ofSelector = node.of !== null ? compileSelector(node.of) : TRUE_FN;

  // TODO(@marvinhagemeister): we should probably cache results here

  return (ctx, id) => {
    const siblings = ctx.getSiblings(id);
    const idx = siblings.indexOf(id);

    if (!node.repeat) {
      return idx === node.stepOffset && next(ctx, id);
    }

    for (let i = 0; i < siblings.length; i++) {
      const n = getNthAnB(node, i);

      if (n > siblings.length - 1) return false;

      const search = siblings[n];
      if (id === search) {
        if (node.of !== null && !ofSelector(ctx, id)) {
          continue;
        } else if (next(ctx, id)) {
          return true;
        }
      } else if (n > idx) {
        return false;
      }
    }

    return false;
  };
}

/**
 * @param {Selector[]} selectors
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchIs(selectors, next) {
  /** @type {MatcherFn[]} */
  const compiled = [];

  for (let i = 0; i < selectors.length; i++) {
    const sel = selectors[i];
    compiled.push(compileSelector(sel));
  }

  return (ctx, id) => {
    for (let i = 0; i < compiled.length; i++) {
      const sel = compiled[i];
      if (sel(ctx, id)) return next(ctx, id);
    }

    return false;
  };
}

/**
 * @param {Selector[]} selectors
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchHas(selectors, next) {
  /** @type {MatcherFn[]} */
  const compiled = [];

  for (let i = 0; i < selectors.length; i++) {
    const sel = selectors[i];
    compiled.push(compileSelector(sel));
  }

  /** @type {Map<number, boolean>} */
  const cache = new Map();

  return (ctx, id) => {
    if (next(ctx, id)) {
      const cached = cache.get(id);
      if (cached !== undefined) return cached;

      const match = ctx.subSelect(compiled, id);
      cache.set(id, match);
      if (match) {
        return true;
      }
    }

    return false;
  };
}

/**
 * @param {Selector[]} selectors
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchNot(selectors, next) {
  /** @type {MatcherFn[]} */
  const compiled = [];

  for (let i = 0; i < selectors.length; i++) {
    const sel = selectors[i];
    compiled.push(compileSelector(sel));
  }

  /** @type {Map<number, boolean>} */
  const cache = new Map();

  return (ctx, id) => {
    if (next(ctx, id)) {
      const cached = cache.get(id);
      if (cached !== undefined) return cached;

      const match = ctx.subSelect(compiled, id);
      cache.set(id, !match);
      if (!match) {
        return true;
      }
    }

    return false;
  };
}

/**
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchDescendant(next) {
  // TODO(@marvinhagemeister): we should probably cache results here
  return (ctx, id) => {
    let current = ctx.getParent(id);
    while (current > 0) {
      if (next(ctx, current)) {
        return true;
      }

      current = ctx.getParent(current);
    }

    return false;
  };
}

/**
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchChild(next) {
  return (ctx, id) => {
    const parent = ctx.getParent(id);
    if (parent === 0) return false;

    return next(ctx, parent);
  };
}

/**
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchAdjacent(next) {
  return (ctx, id) => {
    const siblings = ctx.getSiblings(id);
    const idx = siblings.indexOf(id) - 1;

    if (idx < 0) return false;

    const prev = siblings[idx];
    return next(ctx, prev);
  };
}

/**
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchFollowing(next) {
  return (ctx, id) => {
    const siblings = ctx.getSiblings(id);
    const idx = siblings.indexOf(id) - 1;

    if (idx < 0) return false;

    for (let i = idx; i >= 0; i--) {
      const sib = siblings[i];
      if (next(ctx, sib)) return true;
    }

    return false;
  };
}

/**
 * @param {ElemSelector} part
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchElem(part, next) {
  return (ctx, id) => {
    // Placeholder node cannot be matched
    if (id === 0) return false;
    // Wildcard always matches
    else if (part.wildcard) return next(ctx, id);
    // 0 means it's the placeholder node which
    // can never be matched.
    else if (part.elem === 0) return false;

    const type = ctx.getType(id);
    if (type > 0 && type === part.elem) {
      return next(ctx, id);
    }

    return false;
  };
}

/**
 * @param {FieldSelector} part
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchField(part, next) {
  return (ctx, id) => {
    let child = id;
    let parent = ctx.getParent(id);
    if (parent === 0) return false;

    // Fields are stored left-ro-right but we need to match
    // them right-to-left because we're matching selectors
    // in that direction. Matching right to left is done for
    // performance and reduces the number of potential mismatches.
    for (let i = part.props.length - 1; i >= 0; i--) {
      const prop = part.props[i];
      const value = ctx.getField(parent, prop);

      if (value === -1) return false;
      if (value !== child) return false;

      if (i > 0) {
        child = parent;
        parent = ctx.getParent(parent);
        if (parent === 0) return false;
      }
    }

    return next(ctx, parent);
  };
}

/**
 * @param {AttrExists} attr
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchAttrExists(attr, next) {
  return (ctx, id) => {
    try {
      ctx.getAttrPathValue(id, attr.prop, 0);
      return next(ctx, id);
    } catch (err) {
      if (err === -1) {
        return false;
      }

      throw err;
    }
  };
}

/**
 * @param {AttrBin} attr
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchAttrBin(attr, next) {
  return (ctx, id) => {
    try {
      const value = ctx.getAttrPathValue(id, attr.prop, 0);
      if (!matchAttrValue(attr, value)) return false;
    } catch (err) {
      if (err === -1) {
        return false;
      }
      throw err;
    }
    return next(ctx, id);
  };
}

/**
 * @param {AttrBin} attr
 * @param {*} value
 * @returns {boolean}
 */
function matchAttrValue(attr, value) {
  switch (attr.op) {
    case BinOp.Equal:
      return attr.value instanceof RegExp
        ? attr.value.test(value)
        : value === attr.value;
    case BinOp.NotEqual:
      return attr.value instanceof RegExp
        ? !attr.value.test(value)
        : value !== attr.value;
    case BinOp.Greater:
      return typeof value === "number" && typeof attr.value === "number" &&
        value > attr.value;
    case BinOp.GreaterThan:
      return typeof value === "number" && typeof attr.value === "number" &&
        value >= attr.value;
    case BinOp.Less:
      return typeof value === "number" && typeof attr.value === "number" &&
        value < attr.value;
    case BinOp.LessThan:
      return typeof value === "number" && typeof attr.value === "number" &&
        value <= attr.value;
    default:
      return false;
  }
}
