// Copyright 2018-2026 the Deno authors. MIT license.

// JS glue for the native `CSSStyleSheet` / `CSSRule` implementation in
// css_stylesheet.rs (CSS module scripts, `with { type: "css" }`). The classes
// themselves are implemented in Rust as cppgc objects; this file only wires
// up custom inspect output and WebIDL interface conventions.

(function () {
const { core, primordials } = __bootstrap;
const { CSSRule, CSSStyleSheet } = core.ops;
const {
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  SymbolFor,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);

const CSSRulePrototype = CSSRule.prototype;
const CSSStyleSheetPrototype = CSSStyleSheet.prototype;

function defineCustomInspect(prototype, keys) {
  ObjectDefineProperty(
    prototype,
    SymbolFor("Deno.privateCustomInspect"),
    {
      __proto__: null,
      value: function customInspect(inspect, inspectOptions) {
        return inspect(
          createFilteredInspectProxy({
            object: this,
            evaluate: ObjectPrototypeIsPrototypeOf(prototype, this),
            keys,
          }),
          inspectOptions,
        );
      },
      enumerable: false,
      writable: true,
      configurable: true,
    },
  );
}

defineCustomInspect(CSSRulePrototype, ["cssText"]);
defineCustomInspect(CSSStyleSheetPrototype, ["cssRules"]);

webidl.configureInterface(CSSRule);
webidl.configureInterface(CSSStyleSheet);

return {
  CSSRule,
  CSSRulePrototype,
  CSSStyleSheet,
  CSSStyleSheetPrototype,
};
})();
