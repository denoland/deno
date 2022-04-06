// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// @ts-check
/// <reference path="../../core/internal.d.ts" />
/// <reference path="../../core/lib.deno_core.d.ts" />
/// <reference path="../webidl/internal.d.ts" />
/// <reference path="../web/lib.deno_web.d.ts" />

"use strict";

((window) => {
  const core = Deno.core;
  const webidl = window.__bootstrap.webidl;

  const constructorKey = Symbol();

  const _disabled = Symbol("disabled");

  class StyleSheet {
    [_disabled] = false;

    /** @param [key] {symbol} */
    constructor(key = undefined) {
      if (key !== constructorKey) webidl.illegalConstructor();
      this[webidl.brand] = webidl.brand;
    }

    get type() {
      webidl.assertBranded(this, StyleSheetPrototype);
      return "text/css";
    }

    get disabled() {
      webidl.assertBranded(this, StyleSheetPrototype);
      return this[_disabled];
    }
  }
  const StyleSheetPrototype = StyleSheet.prototype;
  webidl.configurePrototype(StyleSheet);

  const _CSSRules = Symbol("CSS rules");
  const _rulesObject = Symbol("rules object");

  class CSSStyleSheet extends StyleSheet {
    [_CSSRules] = [];
    [_rulesObject] = undefined;

    constructor(options = {}) {
      super(constructorKey);
      const prefix = "Failed to construct 'CSSStyleSheet'";
      options = webidl.converters.CSSStyleSheetInit(options, {
        prefix,
        context: "Argument 1",
      });

      this[_disabled] = options.disabled;
    }

    get cssRules() {
      webidl.assertBranded(this, CSSStyleSheetPrototype);
      if (this[_rulesObject] === undefined) {
        this[_rulesObject] = createCSSRuleList(this[_CSSRules]);
      }
      return this[_rulesObject];
    }

    insertRule(rule, index = 0) {
      webidl.assertBranded(this, CSSStyleSheetPrototype);
      const prefix = "Failed to execute 'insertRule' on 'CSSStyleSheet'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      rule = webidl.converters.CSSOMString(rule, {
        prefix,
        context: "Argument 1",
      });
      index = webidl.converters["unsigned long"](index, {
        prefix,
        context: "Argument 2",
      });

      // TODO: step 2

      const ast = core.opSync("op_css_parse_rule", rule);
      const cssRule = cssRuleFromAst(ast);

      // TODO: step 5 and 6

      this[_CSSRules].splice(index, 0, cssRule);
      return index;
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `CSSStyleSheet ${
        inspect({
          cssRules: this.cssRules,
          disabled: this.disabled,
        })
      }`;
    }
  }
  const CSSStyleSheetPrototype = CSSStyleSheet.prototype;
  webidl.configurePrototype(CSSStyleSheet);

  const _rules = Symbol("rules");

  /**
   * @param {CSSRule[]} rules
   * @returns {CSSRuleList}
   */
  function createCSSRuleList(rules) {
    const list = webidl.createBranded(CSSRuleList);
    list[_rules] = rules;
    return list;
  }

  class CSSRuleList {
    /** @type {CSSRule[]} */
    [_rules] = [];

    constructor() {
      webidl.illegalConstructor();
    }

    // TODO: webidl getter
    /**
     * @param {number} index
     * @returns {CSSRule | undefined}
     */
    item(index) {
      webidl.assertBranded(this, CSSRuleListPrototype);
      const prefix = "Failed to execute 'item' on 'CSSRuleList'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      index = webidl.converters["unsigned long"](index, {
        prefix,
        context: "Argument 1",
      });
      return this[_rules][index];
    }

    get length() {
      webidl.assertBranded(this, CSSRuleListPrototype);
      return this[_rules].length;
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `CSSRuleList ${inspect({ length: this.length })}`;
    }
  }
  const CSSRuleListPrototype = CSSRuleList.prototype;
  webidl.configurePrototype(CSSRuleList);

  const _type = Symbol("type");
  const _cssText = Symbol("css text");
  const _parentRule = Symbol("parent rule");
  const _parentStyleSheet = Symbol("parent style sheet");

  function cssRuleFromAst(ast) {
    switch (ast.kind) {
      case "style":
        return cssStyleRuleFromAst(ast.value);
      default:
        throw "unimplemented";
    }
  }

  class CSSRule {
    constructor() {
      webidl.illegalConstructor();
    }

    get cssText() {
      webidl.assertBranded(this, CSSRulePrototype);
      return this[_cssText];
    }

    get parentRule() {
      webidl.assertBranded(this, CSSRulePrototype);
      return this[_parentRule];
    }

    get parentStyleSheet() {
      webidl.assertBranded(this, CSSRulePrototype);
      return this[_parentStyleSheet];
    }

    get type() {
      webidl.assertBranded(this, CSSRulePrototype);
      return this[_type];
    }

    static get STYLE_RULE() {
      return 1;
    }
    static get CHARSET_RULE() {
      return 2;
    }
    static get IMPORT_RULE() {
      return 3;
    }
    static get MEDIA_RULE() {
      return 4;
    }
    static get FONT_FACE_RULE() {
      return 5;
    }
    static get PAGE_RULE() {
      return 6;
    }
    static get MARGIN_RULE() {
      return 9;
    }
    static get NAMESPACE_RULE() {
      return 10;
    }
    get STYLE_RULE() {
      return 1;
    }
    get CHARSET_RULE() {
      return 2;
    }
    get IMPORT_RULE() {
      return 3;
    }
    get MEDIA_RULE() {
      return 4;
    }
    get FONT_FACE_RULE() {
      return 5;
    }
    get PAGE_RULE() {
      return 6;
    }
    get MARGIN_RULE() {
      return 9;
    }
    get NAMESPACE_RULE() {
      return 10;
    }
  }
  const CSSRulePrototype = CSSRule.prototype;
  webidl.configurePrototype(CSSRule);

  const _selectorText = Symbol("selector text");

  function cssStyleRuleFromAst(ast) {
    return createCSSStyleRule(ast.selector);
  }

  function createCSSStyleRule(selectorText) {
    const rule = webidl.createBranded(CSSStyleRule);
    rule[_selectorText] = selectorText;
    return rule;
  }

  class CSSStyleRule extends CSSRule {
    constructor() {
      webidl.illegalConstructor();
      super();
    }

    get selectorText() {
      webidl.assertBranded(this, CSSStyleRulePrototype);
      return this[_selectorText];
    }

    set selectorText(value) {
      webidl.assertBranded(this, CSSStyleRulePrototype);
      const prefix = "Failed to execute 'selectorText' on 'CSSStyleRule'";
      webidl.requiredArguments(arguments.length, 1, { prefix });
      value = webidl.converters.CSSOMString(value, {
        prefix,
        context: "Argument 1",
      });
      // TODO(lucacasonnato): set it correctly
      throw "unimplmented";
    }

    [Symbol.for("Deno.customInspect")](inspect) {
      return `CSSStyleRule ${inspect({ selectorText: this.selectorText })}`;
    }
  }
  const CSSStyleRulePrototype = CSSStyleRule.prototype;
  webidl.configurePrototype(CSSStyleRule);

  webidl.converters.CSSOMString = webidl.converters.DOMString;

  webidl.converters["CSSStyleSheetInit"] = webidl.createDictionaryConverter(
    "CSSStyleSheetInit",
    [
      // TODO: baseURL?
      // {
      //   key: "media",
      //   converter: webidl.converters.CSSOMString, // TODO: MediaList
      //   defaultValue: "",
      // },
      {
        key: "disabled",
        converter: webidl.converters.boolean,
        defaultValue: false,
      },
    ],
  );

  globalThis.__bootstrap.css = {
    StyleSheet,
    CSSStyleSheet,
    CSSRuleList,
    CSSRule,
  };
})(globalThis);
