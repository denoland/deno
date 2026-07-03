// Copyright 2018-2026 the Deno authors. MIT license.

// In-binary JSX runtime bridge. This module is served by the CLI graph loader
// for the reserved `deno-jsx:preact/jsx-runtime` specifier that Deno uses as
// the default JSX import source. It wraps Preact's JSX runtime so that a
// returned vnode renders to HTML when stringified, which makes the ergonomic
// `new Response(<App />)` form work out of the box (without it, a raw Preact
// vnode stringifies to "[object Object]"). The `npm:` imports below are
// resolved and installed through the normal npm path.

import { Fragment, jsx as _jsx, jsxs as _jsxs } from "npm:preact/jsx-runtime";
import { render } from "npm:preact-render-to-string";

function withHtml(vnode) {
  if (vnode && typeof vnode === "object") {
    Object.defineProperty(vnode, "toString", {
      value: () => render(vnode),
      enumerable: false,
      configurable: true,
    });
  }
  return vnode;
}

export const jsx = (type, props, key) => withHtml(_jsx(type, props, key));
export const jsxs = (type, props, key) => withHtml(_jsxs(type, props, key));
export { Fragment };
