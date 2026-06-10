// Copyright 2018-2026 the Deno authors. MIT license.

// @ts-check
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="./internal.d.ts" />

// Minimal prototype implementation of `CSSStyleSheet` to support CSS module
// scripts (`import sheet from "./a.css" with { type: "css" }`):
// https://html.spec.whatwg.org/multipage/webappapis.html#css-module-script
//
// Deno has no DOM, so a sheet can't be adopted anywhere; the implementation
// is backed by the raw CSS text. `cssRules` performs a naive top-level rule
// split (tracking braces, strings and comments) instead of real CSS parsing,
// which is enough to read rules back out for SSR-style serialization.
// Exposed as a global only when `--unstable-raw-imports` is enabled.

(function () {
const { core, primordials } = __bootstrap;
const {
  ArrayPrototypePush,
  ObjectFreeze,
  ObjectPrototypeIsPrototypeOf,
  PromiseReject,
  PromiseResolve,
  StringPrototypeCharCodeAt,
  StringPrototypeSlice,
  StringPrototypeTrim,
  Symbol,
  SymbolFor,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);

const _text = Symbol("[[text]]");
const _rules = Symbol("[[cssRules]]");
const illegalConstructorKey = Symbol("illegalConstructorKey");

class CSSRule {
  [_text];

  constructor(key = undefined, text = undefined) {
    if (key !== illegalConstructorKey) {
      webidl.illegalConstructor();
    }
    this[webidl.brand] = webidl.brand;
    this[_text] = text;
  }

  get cssText() {
    webidl.assertBranded(this, CSSRulePrototype);
    return this[_text];
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(CSSRulePrototype, this),
        keys: ["cssText"],
      }),
      inspectOptions,
    );
  }
}
webidl.configureInterface(CSSRule);
const CSSRulePrototype = CSSRule.prototype;

const CHAR_LBRACE = 0x7b; // {
const CHAR_RBRACE = 0x7d; // }
const CHAR_SEMICOLON = 0x3b; // ;
const CHAR_SLASH = 0x2f; // /
const CHAR_STAR = 0x2a; // *
const CHAR_QUOTE = 0x22; // "
const CHAR_APOSTROPHE = 0x27; // '
const CHAR_BACKSLASH = 0x5c; // \

/**
 * Splits a style sheet's text into its top-level rules. This is not a real
 * CSS parser: it only tracks brace depth while skipping comments and
 * strings, so each returned chunk is the verbatim text of one top-level
 * rule (or one at-statement like `@import "x";`).
 * @param {string} text
 * @returns {string[]}
 */
function splitTopLevelRules(text) {
  const chunks = [];
  const len = text.length;
  let depth = 0;
  let start = 0;
  let i = 0;
  while (i < len) {
    const c = StringPrototypeCharCodeAt(text, i);
    if (
      c === CHAR_SLASH && i + 1 < len &&
      StringPrototypeCharCodeAt(text, i + 1) === CHAR_STAR
    ) {
      i += 2;
      while (
        i + 1 < len &&
        !(StringPrototypeCharCodeAt(text, i) === CHAR_STAR &&
          StringPrototypeCharCodeAt(text, i + 1) === CHAR_SLASH)
      ) {
        i++;
      }
      i += 2;
      continue;
    }
    if (c === CHAR_QUOTE || c === CHAR_APOSTROPHE) {
      i++;
      while (i < len) {
        const q = StringPrototypeCharCodeAt(text, i);
        if (q === CHAR_BACKSLASH) {
          i += 2;
          continue;
        }
        i++;
        if (q === c) {
          break;
        }
      }
      continue;
    }
    if (c === CHAR_LBRACE) {
      depth++;
    } else if (c === CHAR_RBRACE) {
      if (depth > 0) {
        depth--;
      }
      if (depth === 0) {
        const chunk = StringPrototypeTrim(
          StringPrototypeSlice(text, start, i + 1),
        );
        if (chunk !== "") {
          ArrayPrototypePush(chunks, chunk);
        }
        start = i + 1;
      }
    } else if (c === CHAR_SEMICOLON && depth === 0) {
      const chunk = StringPrototypeTrim(
        StringPrototypeSlice(text, start, i + 1),
      );
      if (chunk !== "") {
        ArrayPrototypePush(chunks, chunk);
      }
      start = i + 1;
    }
    i++;
  }
  const rest = StringPrototypeTrim(StringPrototypeSlice(text, start));
  if (rest !== "") {
    ArrayPrototypePush(chunks, rest);
  }
  return chunks;
}

class CSSStyleSheet {
  [_text] = "";
  [_rules] = null;

  constructor(_options = undefined) {
    // `options` (`media`, `disabled`, `baseURL`) are not supported by this
    // minimal implementation.
    this[webidl.brand] = webidl.brand;
  }

  /**
   * Note: returns a frozen array of `CSSRule` instead of a live
   * `CSSRuleList`.
   */
  get cssRules() {
    webidl.assertBranded(this, CSSStyleSheetPrototype);
    if (this[_rules] === null) {
      const rules = [];
      const chunks = splitTopLevelRules(this[_text]);
      for (let i = 0; i < chunks.length; i++) {
        ArrayPrototypePush(
          rules,
          new CSSRule(illegalConstructorKey, chunks[i]),
        );
      }
      this[_rules] = ObjectFreeze(rules);
    }
    return this[_rules];
  }

  replaceSync(text) {
    webidl.assertBranded(this, CSSStyleSheetPrototype);
    const prefix = "Failed to execute 'replaceSync' on 'CSSStyleSheet'";
    webidl.requiredArguments(arguments.length, 1, prefix);
    text = webidl.converters.DOMString(text, prefix, "Argument 1");
    this[_text] = text;
    this[_rules] = null;
  }

  replace(text) {
    try {
      this.replaceSync(text);
      return PromiseResolve(this);
    } catch (e) {
      return PromiseReject(e);
    }
  }

  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(CSSStyleSheetPrototype, this),
        keys: ["cssRules"],
      }),
      inspectOptions,
    );
  }
}
webidl.configureInterface(CSSStyleSheet);
const CSSStyleSheetPrototype = CSSStyleSheet.prototype;

return {
  CSSRule,
  CSSRulePrototype,
  CSSStyleSheet,
  CSSStyleSheetPrototype,
};
})();
