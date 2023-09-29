// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
/// <reference path="../../core/internal.d.ts" />

const primordials = globalThis.__bootstrap.primordials;
const {
  Array,
  ArrayIsArray,
  ArrayPrototypePush,
  ArrayPrototypePushApply,
  ObjectKeys,
  String,
  StringPrototypeRepeat,
  StringPrototypeSlice,
  SymbolFor,
} = primordials;

/**
 * React adapter to serialize JSX to string
 * @type {import("ext:deno_console/jsx").JSXSerializeAdapter}
 */
export const reactAdapter = {
  getName(vnode) {
    if (typeof vnode.type === "string") return vnode.type;
    if (typeof vnode.type === "function") {
      return vnode.type.displayName || vnode.type.name || "Anonymous";
    }
    if (vnode.type == SymbolFor("react.fragment")) return "Fragment";
    if (vnode.type == SymbolFor("react.suspense")) return "Suspense";
    if (vnode.type == SymbolFor("react.strict_mode")) return "StrictMode";
    if (vnode.type == SymbolFor("react.profiler")) return "Profiler";

    if (
      vnode.type !== null &&
      typeof vnode.type === "object" &&
      typeof vnode.type.$$typeof === "symbol"
    ) {
      const $$typeof = vnode.type.$$typeof;
      if ($$typeof === SymbolFor("react.provider")) {
        const suffix = vnode.type.displayName
          ? "." + vnode.type.displayName
          : "";
        return `Provider${suffix}`;
      } else if ($$typeof === SymbolFor("react.context")) {
        const suffix = vnode.type.displayName
          ? "." + vnode.type.displayName
          : "";
        return `Consumer${suffix}`;
      } else if (
        $$typeof === SymbolFor("react.memo") &&
        typeof vnode.type.type === "function"
      ) {
        const fnName = vnode.type.type.displayName || vnode.type.type.name ||
          "";
        return `Memo${fnName ? `(${fnName})` : ""}`;
      } else if ($$typeof === SymbolFor("react.forward_ref")) {
        const name = vnode.type.render.displayName ||
          vnode.type.render.name ||
          "Anonymous";
        return `ForwardRef(${name})`;
      }
    }

    return "Unknown";
  },
  getTextIfTextNode() {
    // React doesn't have proper text elements afaik
    return null;
  },
  isFragment(vnode) {
    return vnode.type === SymbolFor("react.fragment");
  },
};

/**
 * Preact adapter to serialize JSX to string
 * @type {import("ext:deno_console/jsx").JSXSerializeAdapter}
 */
export const preactAdapter = {
  getName(vnode) {
    if (typeof vnode.type === "string") return vnode.type;
    if (typeof vnode.type === "function") {
      const name = vnode.type.displayName || vnode.type.name || "Anonymous";
      // TODO: Fragments can only be detected by importing them, this is
      // a hacky workaround
      if (name === "k") return "Fragment";

      if ("contextType" in vnode.type) {
        const ct = vnode.type.contextType;
        const name = vnode.type === ct.Consumer ? "Consumer" : "Provider";
        const suffix = ct.displayName ? `.${ct.displayName}` : "";
        return name + suffix;
      }
      return name;
    }
    return "Unknown";
  },
  getTextIfTextNode(vnode) {
    return vnode.type === null ? String(vnode.props.data) : null;
  },
  isFragment(vnode) {
    // TODO: Fragments can only be detected by importing them, this is
    // a hacky workaround
    return typeof vnode.type === "function" && vnode.type.name === "k";
  },
  isValidElement(value) {
    return value !== null && typeof value === "object";
  },
};

function isVNode(x) {
  return (
    x !== null &&
    typeof x === "object" &&
    "type" in x &&
    "props" in x &&
    "key" in x
  );
}

/**
 * @param {number} n
 * @returns {string}
 */
function indent(n) {
  if (n === 0) return "";
  return StringPrototypeRepeat("  ", n);
}

const OTHER = "jsxOther";
const ELEMENT = "jsxElement";
const COMPONENT = "jsxComponent";
const SPECIAL = "special";
const ATTR = "jsxAttribute";

/**
 * Serialize JSX to a pretty formatted string
 * @param {{ stylize: (str: string, color: string) => string }} ctx
 * @param {import("ext:deno_console/jsx").JSXSerializeAdapter} adapter
 * @param {import("ext:deno_console/jsx").SharedVNode} vnode
 * @param {number} level
 * @param {number} limit
 * @returns {string}
 */
export function serialize(ctx, adapter, vnode, level, limit) {
  const space = indent(level);

  const text = adapter.getTextIfTextNode(vnode);
  if (text !== null) {
    return space + text;
  }

  const isKeyed = vnode.key !== null && vnode.key !== undefined;

  const isFragment = adapter.isFragment(vnode);
  const isDomNode = !isFragment && typeof vnode.type === "string";
  let namePretty;

  let TAGS = ELEMENT;
  // Fragments are a special case since they have special nameless
  // syntax `<>`. They can only have a `key` prop in React, but Preact
  // doesn't have that restriction
  if (isFragment) {
    TAGS = isKeyed ? COMPONENT : ELEMENT;
    namePretty = isKeyed ? ctx.stylize("Fragment", COMPONENT) : "";
  } else {
    const name = adapter.getName(vnode);
    // Preact text node
    if (name === null) {
      return space + vnode.props.data;
    }

    if (isDomNode) {
      TAGS = ELEMENT;
      namePretty = ctx.stylize(name, ELEMENT);
    } else {
      TAGS = COMPONENT;
      namePretty = ctx.stylize(name, COMPONENT);
    }
  }

  let out = space + ctx.stylize("<", TAGS) + namePretty;

  if (isKeyed) {
    const value = ctx.stylize(`"${String(vnode.key)}"`, "string");
    out += ` ${ctx.stylize("key", ATTR)}${ctx.stylize("=", OTHER)}${value}`;
  }

  out += serializeProps(
    ctx,
    adapter,
    vnode.props,
    isDomNode,
    level + 1 > limit,
  );

  /** @type {import("ext:deno_console/jsx").NormalizedChild} */
  const children = [];
  /** @type {import("ext:deno_console/jsx").VElement} */
  const rawChildren = vnode.props.children;
  normalizeChildren(children, rawChildren, 0);

  if (children.length > 0) {
    const singleChild = !ArrayIsArray(vnode.props.children);

    out += ctx.stylize(">", TAGS);

    // Single text child may be formatted in same line
    if (level + 1 > limit) {
      if (children.length > 0) {
        out += "...";
      }
    } else if (children.length === 1 && typeof children[0] === "string") {
      const str = children[0];
      const fitsSameLine = str.length < 40;
      out += fitsSameLine ? str : indent(level + 1) + str + "\n";
    } else if (children.length === 1 && typeof children[0] === "function") {
      const fn = children[0];
      out += ctx.stylize("{", OTHER) +
        ctx.stylize(`[Function: ${fn.name || "Anonymous"}]`, SPECIAL) +
        ctx.stylize("}", OTHER);
    } else {
      out += "\n";

      const unwrapped = singleChild ? children[0] : children;

      if (ArrayIsArray(unwrapped)) {
        for (let i = 0; i < unwrapped.length; i++) {
          out += serializeChildren(ctx, adapter, unwrapped[i], level, limit);
        }
      } else {
        out += serializeChildren(ctx, adapter, unwrapped, level, limit);
      }

      out += indent(level);
    }

    out += ctx.stylize("</", TAGS) + namePretty +
      ctx.stylize(">", TAGS);
  } else if (isFragment && !isKeyed) {
    out += ctx.stylize("></>", TAGS);
  } else {
    out += ctx.stylize(" />", TAGS);
  }

  if (level > 0 && level + 1 < limit) {
    out += "\n";
  }
  return out;
}

/**
 * @param {{ stylize: (str: string, color: string) => string }} ctx
 * @param {import("ext:deno_console/jsx").JSXSerializeAdapter} adapter
 * @param {import("ext:deno_console/jsx".NormalizedChild)} child
 * @param {number} level
 * @param {number} limit
 * @returns
 */
function serializeChildren(
  ctx,
  adapter,
  child,
  level,
  limit,
) {
  let out = "";
  if (typeof child === "string") {
    return indent(level + 1) + child + "\n";
  } else if (typeof child === "function") {
    return (
      indent(level + 1) +
      ctx.stylize("{", OTHER) +
      ctx.stylize(`[Function: ${child.name || "Anonymous"}]`, SPECIAL) +
      ctx.stylize("}", OTHER) +
      "\n"
    );
  } else if (Array.isArray(child)) {
    out += indent(level + 1) + "[\n";

    for (let i = 0; i < child.length; i++) {
      const actual = child[i];
      out += serializeChildren(ctx, adapter, actual, level + 1, limit);
      out = StringPrototypeSlice(out, 0, -1) + ctx.stylize(",\n", OTHER);
    }

    out += indent(level + 1) + "]\n";
  } else {
    out += serialize(ctx, adapter, child, level + 1, limit);
  }

  return out;
}

/**
 * Flatten children into an array. Nested arrays are flattened and some
 * values like null + undefined or booleans are filtered out.
 * @param {import("ext:deno_console/jsx").NormalizedChild[]} out
 * @param {import("ext:deno_console/jsx").VElement} child
 * @param {number} level
 */
function normalizeChildren(
  out,
  child,
  level,
) {
  // These are ignored as children
  if (child === null || child === undefined || typeof child === "boolean") {
    return;
  }

  // Preserve nested arrays. Frameworks typically replace that with an
  // implicit Fragment, but showing that would be more confusing than
  // showing the array
  if (ArrayIsArray(child)) {
    /** @type {import("ext:deno_console/jsx").NormalizedChild[]} */
    const out2 = [];
    for (let i = 0; i < child.length; i++) {
      normalizeChildren(out2, child[i], level + 1);
    }

    if (out2.length > 0) {
      if (out2.length === 1 && typeof out2[0] === "string") {
        if (out.length > 0) {
          const last = out[out.length - 1];
          if (typeof last === "string") {
            out[out.length - 1] += out2[0];
            return;
          }
        }

        ArrayPrototypePushApply(out, out2);
      } else if (level === 0) {
        ArrayPrototypePushApply(out, out2);
      } else {
        ArrayPrototypePush(out, out2);
      }
    }

    return;
  } else if (typeof child === "function") {
    ArrayPrototypePush(out, child);
    return;
  }

  // Check if child can be merged into previous child
  if (out.length > 0 && !isVNode(child)) {
    const last = out[out.length - 1];
    if (typeof last === "string") {
      out[out.length - 1] += String(child);
      return;
    }
  }

  ArrayPrototypePush(out, isVNode(child) ? child : String(child));
}

/**
 * Serialize JSX props
 * @param {{ stylize: (str: string, color: string) => string }} ctx
 * @param {import("ext:deno_console/jsx").JSXSerializeAdapter} adapter
 * @param {Record<string, unknown>} props
 * @param {boolean} isDomNode
 * @param {boolean} skipVNodeSerialisation
 * @returns {string}
 */
function serializeProps(
  ctx,
  adapter,
  props,
  isDomNode,
  skipVNodeSerialisation,
) {
  // TODO: Primodals
  const sorted = ObjectKeys(props).sort((a, b) => a.localeCompare(b));
  let out = "";

  for (let i = 0; i < sorted.length; i++) {
    const name = sorted[i];
    const value = props[name];

    // Empty values for DOM nodes
    if (
      name === "children" ||
      name === "key" ||
      name === "ref" ||
      value === undefined ||
      (isDomNode && value === null)
    ) {
      continue;
    } else if (typeof value === "string") {
      out += ` ${ctx.stylize(name, ATTR)}${
        ctx.stylize("=", OTHER)
      }${ctx.stylize(`"${value}"`), "green"}`;
    } else {
      // Complex types
      out += ` ${ctx.stylize(name, ATTR)}`;

      // Truthy boolean values are usually displayed without value
      if (value === true) {
        continue;
      }

      out += ctx.stylize("={", OTHER);

      if (ArrayIsArray(value)) {
        out += ctx.stylize(`Array(${value.length})`, SPECIAL);
      } else if (value instanceof Map) {
        out += ctx.stylize(`Map(${value.size})`, SPECIAL);
      } else if (value instanceof Set) {
        out += ctx.stylize(`Set(${value.size})`, SPECIAL);
      } else if (typeof value === "function") {
        out += ctx.stylize(
          `[Function: ${value.name || "Anonymous"}]`,
          SPECIAL,
        );
      } else if (isVNode(value)) {
        if (skipVNodeSerialisation) {
          out += ctx.stylize("...", OTHER);
        } else {
          out += serialize(ctx, adapter, value, 0, 0);
        }
      } else {
        out += ctx.stylize(String(value), SPECIAL);
      }

      out += ctx.stylize("}", OTHER);
    }
  }

  return out;
}
