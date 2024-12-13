// @ts-check

/** @typedef {import("./internal.d.ts").LintState} LintState */
/** @typedef {import("./internal.d.ts").AstContext} AstContext */
/** @typedef {import("./internal.d.ts").MatchCtx} MatchCtx */
/** @typedef {import("./internal.d.ts").AttrOp} AttrOp */
/** @typedef {import("./internal.d.ts").AttrExists} AttrExists */
/** @typedef {import("./internal.d.ts").AttrBin} AttrBin */
/** @typedef {import("./internal.d.ts").AttrSelector} AttrSelector */
/** @typedef {import("./internal.d.ts").Elem} SelectorPart */
/** @typedef {import("./internal.d.ts").PseudoNthChild} PseudoNthChild */
/** @typedef {import("./internal.d.ts").PseudoHas} PseudoHas */
/** @typedef {import("./internal.d.ts").PseudoNot} PseudoNot */
/** @typedef {import("./internal.d.ts").Relation} SRelation */
/** @typedef {import("./internal.d.ts").Selector} Selector */
/** @typedef {import("./internal.d.ts").SelectorParseCtx} SelectorParseCtx */
/** @typedef {import("./internal.d.ts").ILexer} ILexer */
/** @typedef {import("./internal.d.ts").NextFn} NextFn */
/** @typedef {import("./internal.d.ts").MatcherFn} MatcherFn */
/** @typedef {import("./internal.d.ts").AttrRegex} AttrRegex */

const Char = {
  /**   */
  Space: 32,
  /** ' */
  Bang: 33,
  /** " */
  DoubleQuote: 34,
  /** ' */
  Quote: 39,
  /** ( */
  BraceOpen: 40,
  /** ) */
  BraceClose: 41,
  /** + */
  Plus: 43,
  /** , */
  Comma: 44,
  /** : */
  Colon: 58,
  /** < */
  Less: 60,
  /** = */
  Equal: 61,
  /** > */
  Greater: 62,
  /** [ */
  BracketOpen: 91,
  /** ] */
  BracketClose: 93,
  /** ~ */
  Tilde: 126,
};

const Token = {
  Value: 0,
  Char: 1,
  Attr: 2,
  Pseudo: 3,
  EOF: 4,
};

const AttrOp = {
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
};

/**
 * @param {string} s
 * @returns {number}
 */
function getAttrOp(s) {
  switch (s) {
    case "=":
      return AttrOp.Equal;
    case "!=":
      return AttrOp.NotEqual;
    case ">":
      return AttrOp.Greater;
    case ">=":
      return AttrOp.GreaterThan;
    case "<":
      return AttrOp.Less;
    case "<=":
      return AttrOp.LessThan;
    default:
      throw new Error(`Unknown attribute operator: '${s}'`);
  }
}

/** @implements ILexer */
class Lexer {
  token = Token.Value;
  start = 0;
  end = 0;
  ch = 0;
  i = 0;

  value = "";
  value2 = "";
  op = 0;

  /**
   * @param {string} input
   */
  constructor(input) {
    this.input = input;
  }

  /**
   * @param {number} char
   */
  expect(char) {
    if (this.i >= this.input.length) {
      throw new Error(
        `Unterminated selector:\n\n${this.input}\n${" ".repeat(this.i)}^`,
      );
    }
    const ch = this.input.charCodeAt(this.i);
    if (ch !== char) {
      throw new Error(
        `Expected character '${
          String.fromCharCode(ch)
        }', but got '${ch}'.\n\n${this.input}\n${" ".repeat(this.i)}^`,
      );
    }

    this.i++;
  }

  getChar() {
    return this.input.charCodeAt(this.i);
  }

  getSlice() {
    return this.input.slice(this.start, this.i);
  }

  next() {
    this.value = "";
    this.value2 = "";
    this.op = 0;

    if (this.i >= this.input.length) {
      this.token = Token.EOF;
      return;
    }

    let ch = this.input.charCodeAt(this.i);
    console.log("NEXT", JSON.stringify(String.fromCharCode(ch)));
    switch (ch) {
      case Char.Space:
        while (ch === Char.Space) {
          ch = this.getChar();
          this.i++;
        }

        // Check if this a sibling/descendant selector
        if (ch === Char.Plus || ch === Char.Tilde || ch === Char.Greater) {
          this.start = this.i;
          this.end = this.i;
          this.ch = ch;
          this.token = Token.Char;

          console.log("--> yeah");
          this.i++;
          ch = this.getChar();

          while (ch === Char.Space) {
            ch = this.getChar();
            this.i++;
          }
        } else {
          this.start = this.i;
          this.end = this.i;
          this.ch = Char.Space;
          this.token = Token.Char;
          this.i--;
        }

        break;
      case Char.BracketOpen: {
        this.i++;
        this.start = this.i;

        let hasValue = false;
        while (this.i < this.input.length) {
          ch = this.getChar();
          if (
            ch === Char.Equal || ch === Char.Greater || ch === Char.Less ||
            ch === Char.Bang
          ) {
            this.value = this.getSlice().trim();
            hasValue = true;
            break;
          } else if (ch === Char.BracketClose) {
            this.value = this.getSlice();
            break;
          }

          this.i++;
        }

        if (hasValue) {
          this.start = this.i;
          while (
            ch === Char.Equal || ch === Char.Greater || ch === Char.Less ||
            ch === Char.Bang
          ) {
            this.i++;
            ch = this.getChar();
          }

          this.op = getAttrOp(this.getSlice());

          this.start = this.i;

          while (this.i < this.input.length) {
            ch = this.input.charCodeAt(this.i);
            if (ch === Char.BracketClose) {
              const raw = this.getSlice().trim();
              this.value2 = getFromRawValue(raw);
              break;
            }

            this.i++;
          }
        }

        this.expect(Char.BracketClose);

        this.end = this.i;
        this.token = Token.Attr;

        break;
      }
      case Char.Greater:
      case Char.Plus:
      case Char.Tilde:
      case Char.Comma:
      case Char.BraceClose: {
        const original = ch;
        this.start = this.i;
        this.i++;
        while (this.i < this.input.length) {
          ch = this.getChar();
          if (ch !== Char.Space) {
            break;
          }

          this.i++;
        }
        console.log("char", String.fromCharCode(original));
        this.end = this.i;
        this.token = Token.Char;
        this.ch = original;

        break;
      }

      // Pseudo
      case Char.Colon:
        this.start = this.i;
        this.i++;
        while (this.i < this.input.length) {
          ch = this.getChar();
          if (
            ch === Char.Space || ch === Char.BracketOpen ||
            ch === Char.BraceOpen
          ) {
            break;
          }

          this.i++;
        }

        this.end = this.i;
        this.token = Token.Pseudo;
        this.value = this.getSlice();
        break;

      default:
        this.start = this.i;
        this.i++;

        loop: while (this.i < this.input.length) {
          ch = this.getChar();

          switch (ch) {
            case Char.Space:
            case Char.Comma:
            case Char.Colon:
            case Char.BracketOpen:
            case Char.BraceClose:
            case Char.Greater:
            case Char.Plus:
            case Char.Tilde:
              console.log("BREAK", this.getSlice());
              break loop;
            default:
              this.i++;
          }
        }

        this.end = this.i;
        this.value = this.getSlice();
        console.log({ v: this.value });
        this.token = Token.Value;
    }
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
        const end = raw.lastIndexOf("/", 1);
        if (end === -1) throw new Error(`Invalid RegExp pattern: ${raw}`);
        const pattern = raw.slice(0, end);
        const flags = end < raw.length - 1 ? raw.slice(end) : undefined;
        return new RegExp(pattern, flags);
      } else if (NUMBER_REG.test(raw)) {
        return Number(raw);
      } else if (BIGINT_REG.test(raw)) {
        return BigInt(raw.slice(0, -1));
      }

      return raw;
  }
}

const ELEM_NODE = 1;
const RELATION_NODE = 2;
const ATTR_EXISTS_NODE = 3;
const ATTR_BIN_NODE = 4;
const ATTR_REGEX_NODE = 5;
const PSEUDO_NODE_NTH_CHILD = 6;
const PSEUDO_NODE_HAS = 7;
const PSEUDO_NODE_NOT = 8;
const PSEUDO_FIRST_CHILD = 9;
const PSEUDO_LAST_CHILD = 10;

/**
 * @param {string} input
 * @param {Record<string, number>} astNodes
 * @param {Record<string, number>} astAttrs
 * @returns {Selector[]}
 */
export function parseSelector(input, astNodes, astAttrs) {
  /** @type {Selector[]} */
  const result = [];

  /** @type {Selector} */
  let current = [];

  const lex = new Lexer(input);
  lex.next();

  while (lex.token !== Token.EOF) {
    console.log(
      lex.token,
      JSON.stringify(String.fromCharCode(lex.ch)),
      Token,
      result,
      current,
    );
    if (lex.token === Token.Value) {
      const name = lex.value;
      const wildcard = name === "*";

      let elem = 0;
      if (!wildcard) {
        elem = astNodes[name];
        if (elem === undefined) {
          throw new Error(`Unkown element: ${name}`);
        }
      }

      current.push({
        type: ELEM_NODE,
        elem,
        debug: name,
        wildcard,
      });
    } else if (lex.token === Token.Attr) {
      const name = lex.value;
      const id = astAttrs[name];
      if (id === undefined) {
        console.log(lex);
        throw new Error(`Unknown attribute: ${name}`);
      }

      if (lex.value2 === "") {
        current.push({
          type: ATTR_EXISTS_NODE,
          prop: id,
          debug: lex.value,
        });
      } else {
        current.push({
          type: ATTR_BIN_NODE,
          prop: id,
          op: lex.op,
          debug: lex.value,
          value: lex.value2,
        });
      }
    } else if (lex.token === Token.Pseudo) {
      console.log("PSEUDO", lex);
      switch (lex.value) {
        case ":first-child":
          current.push({
            type: PSEUDO_FIRST_CHILD,
          });
          break;
        case ":last-child":
          current.push({
            type: PSEUDO_LAST_CHILD,
          });
          break;
        case ":nth-child":
          lex.expect(Char.BraceOpen);

          console.log("nth", lex);
          console.log(lex.getSlice());

          current.push({
            type: PSEUDO_NODE_NTH_CHILD,
          });

          break;
        default:
          throw new Error(`Unknown pseudo selector: '${lex.value}'`);
      }
    } else if (lex.ch === Char.Comma) {
      result.push(current);
      current = [];
    } else if (
      lex.ch === Char.Space || lex.ch === Char.Plus || lex.ch === Char.Tilde ||
      lex.ch === Char.Greater
    ) {
      current.push({
        type: RELATION_NODE,
        op: lex.ch,
        debug: JSON.stringify(String.fromCharCode(lex.ch)),
      });
    }

    lex.next();
  }

  if (current.length > 0) {
    result.push(current);
  }

  console.log(lex);
  console.log("--> SELECTORS", result);

  return result;
}

const TRUE_FN = () => true;

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
      case RELATION_NODE:
        switch (node.op) {
          case Char.Space:
            fn = matchDescendant(fn);
            break;
          case Char.Greater:
            fn = matchChild(fn);
            break;
          case Char.Plus:
            fn = matchAdjacent(fn);
            break;
          case Char.Tilde:
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
      case ATTR_REGEX_NODE:
        fn = matchAttrRegex(node, fn);
        break;
      case PSEUDO_FIRST_CHILD:
        fn = matchFirstChild(fn);
        break;
      case PSEUDO_LAST_CHILD:
        fn = matchLastChild(fn);
        break;
      case PSEUDO_NODE_NTH_CHILD:
        fn = matchNthChild(node, fn);
        break;
      case PSEUDO_NODE_HAS:
        // FIXME
        // fn = matchIs(part, fn);
        throw new Error("TODO: :has");
      case PSEUDO_NODE_NOT:
        fn = matchNot(node.selector, fn);
        break;
    }
  }

  return fn;
}

/**
 * @param {NextFn} next
 * @returns {MatcherFn}
 */
function matchFirstChild(next) {
  return (ctx, id) => {
    const parent = ctx.getParent(id);
    const first = ctx.getFirstChild(parent);
    return first === id && next(ctx, first);
  };
}

/**
 * @param {NextFn} next
 * @returns {MatcherFn}
 */
function matchLastChild(next) {
  return (ctx, id) => {
    const parent = ctx.getParent(id);
    const last = ctx.getLastChild(parent);
    return last === id && next(ctx, id);
  };
}

/**
 * @param {PseudoNthChild} node
 * @param {NextFn} next
 * @returns {MatcherFn}
 */
function matchNthChild(node, next) {
  const ofSelector = node.of !== null ? compileSelector(node.of) : TRUE_FN;

  return (ctx, id) => {
    const siblings = ctx.getSiblings(id);

    if (node.backward) {
      for (
        let i = siblings.length - 1 - node.stepOffset;
        i < siblings.length;
        i += node.step
      ) {
        const sib = siblings[i];

        if (sib !== id) {
          continue;
        }

        if (node.of !== null && !ofSelector(ctx, sib)) {
          continue;
        } else if (next(ctx, sib)) {
          return true;
        }
      }

      return false;
    }

    for (let i = node.stepOffset; i < siblings.length; i += node.step) {
      const sib = siblings[i];

      if (node.of !== null && !ofSelector(ctx, sib)) {
        continue;
      } else if (next(ctx, sib)) {
        return true;
      }
    }

    return false;
  };
}

/**
 * @param {Selector[]} selectors
 * @param {NextFn} next
 * @returns {MatcherFn}
 */
function matchNot(selectors, next) {
  /** @type {MatcherFn[]} */
  const compiled = [];

  for (let i = 0; i < selectors.length; i++) {
    const sel = selectors[i];
    compiled.push(compileSelector(sel));
  }

  return (ctx, id) => {
    for (let i = 0; i < compiled.length; i++) {
      const fn = compiled[i];
      if (fn(ctx, id)) {
        return false;
      }
    }

    return next(ctx, id);
  };
}

/**
 * @param {NextFn} next
 * @returns {MatcherFn}
 */
function matchDescendant(next) {
  return (ctx, id) => {
    let current = ctx.getParent(id);
    while (current > -1) {
      if (next(ctx, current)) {
        return true;
      }

      current = ctx.getParent(current);
    }

    return false;
  };
}

/**
 * @param {NextFn} next
 * @returns {MatcherFn}
 */
function matchChild(next) {
  return (ctx, id) => {
    const parent = ctx.getParent(id);
    if (parent < 0) return false;

    return next(ctx, parent);
  };
}

/**
 * @param {NextFn} next
 * @returns {MatcherFn}
 */
function matchAdjacent(next) {
  return (ctx, id) => {
    const parent = ctx.getParent(id);
    if (parent < 0) return false;

    const prev = ctx.getSiblingBefore(parent, id);
    if (prev < 0) return false;

    return next(ctx, prev);
  };
}

/**
 * @param {NextFn} next
 * @returns {MatcherFn}
 */
function matchFollowing(next) {
  return (ctx, id) => {
    const parent = ctx.getParent(id);
    if (parent < 0) return false;

    let prev = ctx.getSiblingBefore(parent, id);
    while (prev > -1) {
      if (next(ctx, prev)) {
        return true;
      }

      prev = ctx.getSiblingBefore(parent, prev);
    }

    return false;
  };
}

/**
 * @param {SelectorPart} part
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchElem(part, next) {
  return (ctx, id) => {
    if (part.wildcard) return next(ctx, id);
    const type = ctx.getType(id);
    if (type > -1 && type === part.elem) return next(ctx, id);

    return false;
  };
}

/**
 * @param {AttrExists} attr
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchAttrExists(attr, next) {
  return (ctx, id) => {
    return ctx.hasAttr(id, attr.prop) ? next(ctx, id) : false;
  };
}

/**
 * @param {AttrBin} attr
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchAttrBin(attr, next) {
  return (ctx, id) => {
    if (!ctx.hasAttr(id, attr.prop)) return false;
    const value = ctx.getAttrValue(id, attr.prop);
    if (!matchAttrValue(attr, value)) return false;
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
    case AttrOp.Equal:
      return value === attr.value;
    case AttrOp.NotEqual:
      return value !== attr.value;
    case AttrOp.Greater:
      return typeof value === "number" && typeof attr.value === "number" &&
        value > attr.value;
    case AttrOp.GreaterThan:
      return typeof value === "number" && typeof attr.value === "number" &&
        value >= attr.value;
    case AttrOp.Less:
      return typeof value === "number" && typeof attr.value === "number" &&
        value < attr.value;
    case AttrOp.LessThan:
      return typeof value === "number" && typeof attr.value === "number" &&
        value <= attr.value;
    default:
      return false;
  }
}

/**
 * @param {AttrRegex} attr
 * @param {MatcherFn} next
 * @returns {MatcherFn}
 */
function matchAttrRegex(attr, next) {
  return (ctx, id) => {
    const value = ctx.getAttrValue(id, attr.prop);
    if (!attr.value.test(String(value))) {
      return false;
    }

    return next(ctx, id);
  };
}
