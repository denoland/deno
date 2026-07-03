// Copyright 2018-2026 the Deno authors. MIT license.

// Development variant of the in-binary JSX runtime bridge. Served by the CLI
// graph loader for `deno-jsx:preact/jsx-dev-runtime`. See `jsx_bridge.js` for
// details on why the vnode is given an HTML-rendering `toString`.

import { Fragment, jsxDEV as _jsxDEV } from "npm:preact/jsx-runtime";
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

export const jsxDEV = (type, props, key, isStaticChildren, source, self) =>
  withHtml(_jsxDEV(type, props, key, isStaticChildren, source, self));
export { Fragment };
