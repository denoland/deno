// Copyright 2018-2026 the Deno authors. MIT license.

// Internal-only: used by tools/lint.js on runtime/ and ext/ bootstrap code.

const GLOBAL_TARGETS = new Set([
  "isFinite",
  "isNaN",
  "decodeURI",
  "decodeURIComponent",
  "encodeURI",
  "encodeURIComponent",
  "eval",
  "parseFloat",
  "parseInt",
  "queueMicrotask",
  "Atomics",
  "JSON",
  "Math",
  "Reflect",
  "AggregateError",
  "Array",
  "ArrayBuffer",
  "BigInt",
  "BigInt64Array",
  "Boolean",
  "DataView",
  "Date",
  "Error",
  "EvalError",
  "FinalizationRegistry",
  "Float32Array",
  "Float64Array",
  "Function",
  "Int16Array",
  "Int32Array",
  "Int8Array",
  "Map",
  "Number",
  "Object",
  "Promise",
  "Proxy",
  "RangeError",
  "ReferenceError",
  "RegExp",
  "Set",
  "SharedArrayBuffer",
  "String",
  "Symbol",
  "SyntaxError",
  "TypeError",
  "Uint8Array",
  "Uint16Array",
  "Uint32Array",
  "Uint8ClampedArray",
  "URIError",
  "WeakMap",
  "WeakRef",
  "WeakSet",
]);

const UNSAFE_CONSTRUCTOR_TARGETS = new Set([
  "FinalizationRegistry",
  "Map",
  "RegExp",
  "Set",
  "WeakMap",
  "WeakRef",
  "WeakSet",
]);

const UNSAFE_FUNCTION_TARGETS = new Set([
  "PromiseAll",
  "PromiseAllSettled",
  "PromiseAny",
  "PromiseRace",
  "PromisePrototypeFinally",
]);

const METHOD_TARGETS = new Set([
  // Generic
  "toLocaleString",
  "toString",
  "valueOf",
  // Object
  "hasOwnProperty",
  "isPrototypeOf",
  "propertyIsEnumerable",
  // Function
  "apply",
  "bind",
  "call",
  // Number
  "toExponential",
  "toFixed",
  "toPrecision",
  // Date
  "getDate",
  "getDay",
  "getFullYear",
  "getHours",
  "getMilliseconds",
  "getMinutes",
  "getMonth",
  "getSeconds",
  "getTime",
  "getTimezoneOffset",
  "getUTCDate",
  "getUTCDay",
  "getUTCFullYear",
  "getUTCHours",
  "getUTCMilliseconds",
  "getUTCMinutes",
  "getUTCMonth",
  "getUTCSeconds",
  "getYear",
  "setDate",
  "setFullYear",
  "setHours",
  "setMilliseconds",
  "setMinutes",
  "setMonth",
  "setSeconds",
  "setTime",
  "setUTCDate",
  "setUTCFullYear",
  "setUTCHours",
  "setUTCMilliseconds",
  "setUTCMinutes",
  "setUTCMonth",
  "setUTCSeconds",
  "setYear",
  "toDateString",
  "toISOString",
  "toJSON",
  "toLocaleDateString",
  "toLocaleTimeString",
  "toTimeString",
  "toUTCString",
  // String, Array
  "at",
  "concat",
  "slice",
  "includes",
  "indexOf",
  "lastIndexOf",
  // Array, TypedArray
  "copyWithin",
  "entries",
  "every",
  "fill",
  "filter",
  "find",
  "findIndex",
  "findLast",
  "findLastIndex",
  "flat",
  "flatMap",
  "forEach",
  "join",
  "keys",
  "map",
  "pop",
  "push",
  "reduce",
  "reduceRight",
  "reverse",
  "shift",
  "some",
  "sort",
  "toReversed",
  "toSorted",
  "unshift",
  "values",
  "with",
  // String
  "charAt",
  "charCodeAt",
  "codePointAt",
  "endsWith",
  "localeCompare",
  "match",
  "matchAll",
  "normalize",
  "padEnd",
  "padStart",
  "repeat",
  "replace",
  "replaceAll",
  "search",
  "split",
  "startsWith",
  "substring",
  "toLocaleLowerCase",
  "toLocaleUpperCase",
  "toLowerCase",
  "toUpperCase",
  "trim",
  "trimEnd",
  "trimStart",
  // Array
  "splice",
  "toSpliced",
  // ArrayBuffer
  "resize",
  "transfer",
  "transferToFixedLength",
  // SharedArrayBuffer
  "grow",
  // TypedArray: avoid false positives for Map, Set, WeakMap, and WeakSet
  // "set",
  // DataView
  "getBigInt64",
  "getBigUint64",
  "getFloat32",
  "getFloat64",
  "getInt8",
  "getInt16",
  "getInt32",
  "getUint8",
  "getUint16",
  "getUint32",
  "setBigInt64",
  "setBigUint64",
  "setFloat32",
  "setFloat64",
  "setInt8",
  "setInt16",
  "setInt32",
  "setUint8",
  "setUint16",
  "setUint32",
  // Iterator, Generator
  "next",
  "return",
  "throw",
  // Promise
  "catch",
  "finally",
  "then",
]);

const GETTER_TARGETS = new Set([
  // Symbol
  "description",
  // ArrayBuffer, TypedArray, DataView
  "buffer",
  "byteLength",
  "byteOffset",
  // ArrayBuffer, SharedArrayBuffer
  "maxByteLength",
  // ArrayBuffer
  "detached",
  "resizable",
  // SharedArrayBuffer
  "growable",
  // TypedArray: avoid false positives for Array
  // "length",
]);

export const MSG = {
  GlobalIntrinsic: "Don't use the global intrinsic",
  UnsafeIntrinsic: "Don't use the unsafe intrinsic",
  DefineProperty: "Use null [[prototype]] object in the define property",
  ObjectAssignInDefaultParameter:
    "Use null [[prototype]] object in the default parameter",
  Iterator: "Don't use iterator protocol directly",
  RegExp: "Don't use RegExp literal directly",
  InstanceOf: "Don't use `instanceof` operator",
  In: "Don't use `in` operator",
} as const;

export const HINT = {
  GlobalIntrinsic: "Instead use the equivalent from the `primordials` object",
  UnsafeIntrinsic: "Instead use the safe wrapper from the `primordials` object",
  NullPrototypeObjectLiteral: "Add `__proto__: null` to this object literal",
  SafeIterator: "Wrap a SafeIterator from the `primordials` object",
  SafeRegExp: "Wrap `SafeRegExp` from the `primordials` object",
  ObjectPattern: "Instead use the object pattern destructuring assignment",
  InstanceOf:
    "Instead use `ObjectPrototypeIsPrototypeOf` from the `primordials` object",
  In:
    "Instead use either `ObjectHasOwn` or `ReflectHas` from the `primordials` object",
} as const;

type Node = Deno.lint.Node;
type Identifier = Deno.lint.Identifier;
type ObjectExpression = Deno.lint.ObjectExpression;

interface Scope {
  parent: Scope | null;
  bindings: Set<string>;
  range: Deno.lint.Range;
  children: Scope[];
}

function rangeContains(
  outer: Deno.lint.Range,
  inner: Deno.lint.Range,
): boolean {
  return outer[0] <= inner[0] && inner[1] <= outer[1];
}

function posInRange(range: Deno.lint.Range, pos: number): boolean {
  return range[0] <= pos && pos < range[1];
}

function addBindingPattern(scope: Scope, node: Node | null | undefined): void {
  if (!node) return;
  switch (node.type) {
    case "Identifier":
      scope.bindings.add(node.name);
      break;
    case "ObjectPattern":
      for (const prop of node.properties) {
        if (prop.type === "Property") {
          addBindingPattern(scope, prop.value as Node);
        } else if (prop.type === "RestElement") {
          addBindingPattern(scope, prop.argument);
        }
      }
      break;
    case "ArrayPattern":
      for (const el of node.elements) {
        if (el) addBindingPattern(scope, el);
      }
      break;
    case "AssignmentPattern":
      addBindingPattern(scope, node.left);
      break;
    case "RestElement":
      addBindingPattern(scope, node.argument);
      break;
    case "MemberExpression":
      // e.g. ([a.b] = ...) assignment pattern LHS — not a binding
      break;
  }
}

function collectBindingsFromParams(
  scope: Scope,
  params: Deno.lint.Parameter[],
): void {
  for (const param of params) {
    if (param.type === "TSParameterProperty") {
      addBindingPattern(scope, param.parameter as Node);
    } else {
      addBindingPattern(scope, param);
    }
  }
}

/** Build a scope tree so we can detect shadowed globals (e.g. `const Array = ...`). */
function buildScopeTreeFixed(ast: Deno.lint.Program): Scope {
  type ScopeEx = Scope & { isFn: boolean };
  function create(
    parent: ScopeEx | null,
    range: Deno.lint.Range,
    isFn: boolean,
  ): ScopeEx {
    const scope: ScopeEx = {
      parent,
      bindings: new Set(),
      range,
      children: [],
      isFn,
    };
    if (parent) parent.children.push(scope);
    return scope;
  }

  function varTarget(scope: ScopeEx): ScopeEx {
    let cur: ScopeEx | null = scope;
    while (cur) {
      if (cur.isFn || cur.parent === null) return cur;
      cur = cur.parent as ScopeEx | null;
    }
    return scope;
  }

  const root = create(null, ast.range, true);

  function visit(node: Node | null | undefined, scope: ScopeEx): void {
    if (!node || typeof node !== "object" || typeof node.type !== "string") {
      return;
    }
    switch (node.type) {
      case "FunctionDeclaration": {
        if (node.id) scope.bindings.add(node.id.name);
        const inner = create(scope, node.range, true);
        collectBindingsFromParams(inner, node.params);
        if (node.body) {
          // Body block should not create an extra scope for params visibility;
          // but block-level let/const inside body need block scopes.
          // Visit body statements directly in function scope (not a nested
          // BlockStatement scope for the outer body braces matching params).
          // Actually ES: function body is a block scope separate for let/const.
          // Params are in the function scope. let in body is in body block.
          visit(node.body, inner);
        }
        return;
      }
      case "FunctionExpression": {
        const inner = create(scope, node.range, true);
        if (node.id) inner.bindings.add(node.id.name);
        collectBindingsFromParams(inner, node.params);
        if (node.body) visit(node.body, inner);
        return;
      }
      case "ArrowFunctionExpression": {
        const inner = create(scope, node.range, true);
        collectBindingsFromParams(inner, node.params);
        if (node.body.type === "BlockStatement") {
          visit(node.body, inner);
        } else {
          visit(node.body, inner);
        }
        return;
      }
      case "ClassDeclaration": {
        if (node.id) scope.bindings.add(node.id.name);
        const inner = create(scope, node.range, true);
        for (const el of node.body.body) visit(el as Node, inner);
        return;
      }
      case "ClassExpression": {
        const inner = create(scope, node.range, true);
        if (node.id) inner.bindings.add(node.id.name);
        for (const el of node.body.body) visit(el as Node, inner);
        return;
      }
      case "CatchClause": {
        const inner = create(scope, node.range, false);
        addBindingPattern(inner, node.param);
        // catch body is a block
        visit(node.body, inner);
        return;
      }
      case "BlockStatement": {
        const inner = create(scope, node.range, false);
        for (const stmt of node.body) visit(stmt, inner);
        return;
      }
      case "StaticBlock": {
        const inner = create(scope, node.range, false);
        // StaticBlock.body is Statement[], but plugin host proxies can
        // occasionally surface a non-iterable value — walk defensively.
        const body = (node as Deno.lint.StaticBlock).body;
        if (Array.isArray(body)) {
          for (const stmt of body) visit(stmt, inner);
        } else {
          walkChildren(node, (child) => visit(child, inner));
        }
        return;
      }
      case "PropertyDefinition":
      case "MethodDefinition": {
        // Keys are bindings/names, not references — don't walk into key as a
        // free identifier for scope purposes; still walk computed keys + value.
        if (node.computed) visit(node.key as Node, scope);
        if ("value" in node && node.value) visit(node.value as Node, scope);
        return;
      }
      case "ForStatement": {
        const inner = create(scope, node.range, false);
        if (node.init) visit(node.init as Node, inner);
        if (node.test) visit(node.test, inner);
        if (node.update) visit(node.update, inner);
        visit(node.body, inner);
        return;
      }
      case "ForInStatement":
      case "ForOfStatement": {
        const inner = create(scope, node.range, false);
        visit(node.left as Node, inner);
        visit(node.right, inner);
        visit(node.body, inner);
        return;
      }
      case "VariableDeclaration": {
        const target = node.kind === "var" ? varTarget(scope) : scope;
        for (const decl of node.declarations) {
          addBindingPattern(target, decl.id);
          if (decl.init) visit(decl.init, scope);
        }
        return;
      }
      case "ImportDeclaration": {
        for (const spec of node.specifiers) {
          scope.bindings.add(spec.local.name);
        }
        return;
      }
      default:
        walkChildren(node, (child) => visit(child, scope));
    }
  }

  for (const stmt of ast.body) visit(stmt, root);
  return root;
}

function isAstNode(val: unknown): val is Node {
  if (val === null || typeof val !== "object") return false;
  const type = (val as { type?: unknown }).type;
  const range = (val as { range?: unknown }).range;
  return typeof type === "string" && Array.isArray(range) && range.length === 2;
}

/** Walk child AST nodes. Lint AST fields live on the prototype, so we cannot
 * use Object.keys — use for…in and only follow values that look like nodes. */
function walkChildren(node: Node, fn: (child: Node) => void): void {
  const rec = node as unknown as Record<string, unknown>;
  for (const key in rec) {
    if (key === "parent" || key === "range" || key === "type") continue;
    const val = rec[key];
    if (!val || typeof val !== "object") continue;
    if (Array.isArray(val)) {
      for (const item of val) {
        if (isAstNode(item)) fn(item);
      }
    } else if (isAstNode(val)) {
      fn(val);
    }
  }
}

function getParent(node: Node): Node | null {
  return ("parent" in node ? node.parent : null) as Node | null;
}

function findScope(root: Scope, pos: number): Scope {
  let current = root;
  let found = true;
  while (found) {
    found = false;
    for (const child of current.children) {
      if (posInRange(child.range, pos)) {
        current = child;
        found = true;
        break;
      }
    }
  }
  return current;
}

function isShadowed(root: Scope, name: string, pos: number): boolean {
  let scope: Scope | null = findScope(root, pos);
  while (scope) {
    if (scope.bindings.has(name)) return true;
    scope = scope.parent;
  }
  return false;
}

function isNullLiteral(node: Node): boolean {
  if (node.type !== "Literal") return false;
  // Avoid `"regex" in node` — lint AST nodes are proxies and may report
  // inherited keys. Check the actual regex field / value / raw instead.
  const lit = node as Deno.lint.Literal & { regex?: unknown; raw?: string };
  if (lit.regex) return false;
  return lit.value === null || lit.raw === "null";
}

function isNullProto(objectLit: ObjectExpression): boolean {
  for (const prop of objectLit.properties) {
    if (prop.type !== "Property") continue;
    if (prop.computed || prop.method || prop.kind !== "init") continue;
    const key = prop.key;
    let keyName: string | null = null;
    if (key.type === "Identifier") keyName = key.name;
    else if (key.type === "Literal" && typeof key.value === "string") {
      keyName = key.value;
    }
    if (keyName !== "__proto__") continue;
    if (isNullLiteral(prop.value as Node)) return true;
  }
  return false;
}

function insideVarDeclLhsOrMemberExprOrPropOrTypeRef(
  ident: Identifier,
): boolean {
  let node: Node | null = ident;
  while (node) {
    const parent: Node | null = getParent(node);
    if (!parent) break;

    if (parent.type === "MemberExpression") {
      return true;
    }
    if (parent.type === "VariableDeclarator") {
      // ident is in the binding (LHS)
      if (rangeContains(parent.id.range, ident.range)) return true;
    }
    // Names that are not free global references:
    // - object-literal keys, class fields/methods
    // - TypeScript type-only keys (no runtime pollution surface)
    if (
      parent.type === "Property" ||
      parent.type === "PropertyDefinition" ||
      parent.type === "MethodDefinition" ||
      parent.type === "TSPropertySignature" ||
      parent.type === "TSMethodSignature" ||
      parent.type === "TSTypeReference"
    ) {
      if (parent.type === "TSTypeReference") {
        if (rangeContains(parent.typeName.range, ident.range)) return true;
      } else if (
        "computed" in parent && !parent.computed &&
        "key" in parent && parent.key &&
        rangeContains(parent.key.range, ident.range)
      ) {
        return true;
      }
    }

    node = parent;
  }
  return false;
}

function insideParam(node: Node): boolean {
  let cur: Node | null = node;
  while (cur) {
    const parent: Node | null = getParent(cur);
    if (!parent) return false;
    // Function-like params arrays
    if (
      (parent.type === "FunctionDeclaration" ||
        parent.type === "FunctionExpression" ||
        parent.type === "ArrowFunctionExpression") &&
      parent.params.some((p) => rangeContains(p.range, node.range))
    ) {
      return true;
    }
    if (
      parent.type === "CatchClause" &&
      parent.param &&
      rangeContains(parent.param.range, node.range)
    ) {
      return true;
    }
    cur = parent;
  }
  return false;
}

function memberPropName(
  member: Deno.lint.MemberExpression,
): string | null {
  if (member.computed) return null;
  if (member.property.type === "Identifier") return member.property.name;
  return null;
}

function report(
  ctx: Deno.lint.RuleContext,
  node: Node,
  message: string,
  hint: string,
): void {
  ctx.report({ node, message, hint });
}

const plugin: Deno.lint.Plugin = {
  name: "deno-internal",
  rules: {
    "prefer-primordials": {
      create(context) {
        const scopeRoot = buildScopeTreeFixed(context.sourceCode.ast);

        return {
          Identifier(node) {
            if (insideVarDeclLhsOrMemberExprOrPropOrTypeRef(node)) {
              return;
            }

            const name = node.name;
            const pos = node.range[0];

            if (
              GLOBAL_TARGETS.has(name) &&
              !isShadowed(scopeRoot, name, pos)
            ) {
              report(
                context,
                node,
                MSG.GlobalIntrinsic,
                HINT.GlobalIntrinsic,
              );
            }

            if (
              UNSAFE_CONSTRUCTOR_TARGETS.has(name) &&
              node.parent?.type === "NewExpression" &&
              (node.parent as Deno.lint.NewExpression).callee === node
            ) {
              report(
                context,
                node,
                MSG.UnsafeIntrinsic,
                HINT.UnsafeIntrinsic,
              );
            }

            if (
              UNSAFE_FUNCTION_TARGETS.has(name) &&
              node.parent?.type === "CallExpression" &&
              (node.parent as Deno.lint.CallExpression).callee === node
            ) {
              report(
                context,
                node,
                MSG.UnsafeIntrinsic,
                HINT.UnsafeIntrinsic,
              );
            }

            if (
              name === "ObjectDefineProperty" ||
              name === "ReflectDefineProperty"
            ) {
              if (
                node.parent?.type === "CallExpression" &&
                (node.parent as Deno.lint.CallExpression).callee === node
              ) {
                const arg = node.parent.arguments[2];
                if (
                  arg && arg.type === "ObjectExpression" && !isNullProto(arg)
                ) {
                  report(
                    context,
                    arg,
                    MSG.DefineProperty,
                    HINT.NullPrototypeObjectLiteral,
                  );
                }
              }
            }

            if (name === "ObjectDefineProperties") {
              if (
                node.parent?.type === "CallExpression" &&
                (node.parent as Deno.lint.CallExpression).callee === node
              ) {
                const arg = node.parent.arguments[1];
                if (arg && arg.type === "ObjectExpression") {
                  for (const prop of arg.properties) {
                    if (prop.type !== "Property") continue;
                    if (
                      prop.value.type === "ObjectExpression" &&
                      !isNullProto(prop.value)
                    ) {
                      report(
                        context,
                        prop.value,
                        MSG.DefineProperty,
                        HINT.NullPrototypeObjectLiteral,
                      );
                    }
                  }
                }
              }
            }
          },

          MemberExpression(node) {
            // Array literal method/property access
            if (node.object.type === "ArrayExpression") {
              report(
                context,
                node,
                MSG.GlobalIntrinsic,
                HINT.GlobalIntrinsic,
              );
              return;
            }

            // Don't check non-root elements in chained member expressions
            // e.g. `bar.baz` in `foo.bar.baz` — only the full chain is checked
            // for global targets; method/getter checks still apply below.
            if (
              node.parent?.type !== "MemberExpression" &&
              node.object.type === "Identifier" &&
              GLOBAL_TARGETS.has(node.object.name)
            ) {
              report(
                context,
                node,
                MSG.GlobalIntrinsic,
                HINT.GlobalIntrinsic,
              );
              return;
            }

            const propName = memberPropName(node);
            if (!propName) return;

            // Both `foo.bar()` and `fn(foo.bar)` have CallExpression as parent.
            // Only treat the callee case as a call (args still need getter
            // checks). Optional calls (`foo.bar?.()`) still look up methods on
            // the receiver prototype chain, so flag them for pollution resistance.
            const isCallCallee = node.parent?.type === "CallExpression" &&
              (node.parent as Deno.lint.CallExpression).callee === node;

            if (isCallCallee && METHOD_TARGETS.has(propName)) {
              report(
                context,
                node,
                MSG.GlobalIntrinsic,
                HINT.GlobalIntrinsic,
              );
            }

            const isAssignLeft = node.parent?.type === "AssignmentExpression" &&
              (node.parent as Deno.lint.AssignmentExpression).left === node;

            if (
              !isAssignLeft &&
              !isCallCallee &&
              GETTER_TARGETS.has(propName)
            ) {
              report(
                context,
                node,
                MSG.GlobalIntrinsic,
                HINT.GlobalIntrinsic,
              );
            }
          },

          ObjectExpression(node) {
            if (isNullProto(node)) return;
            const parent = node.parent;
            if (!parent) return;

            // Default parameter object: `o = {}` or `{ o = {} }`
            const isDefaultObject = parent.type === "AssignmentPattern" &&
              parent.right === node;

            if (!isDefaultObject) return;
            if (!insideParam(node)) return;

            report(
              context,
              node,
              MSG.ObjectAssignInDefaultParameter,
              HINT.NullPrototypeObjectLiteral,
            );
          },

          SpreadElement(node) {
            // Object spreads are allowed; only array/call/new argument spreads
            // use iterator protocol unsafely.
            if (node.parent?.type === "ObjectExpression") return;
            if (node.argument.type === "NewExpression") return;
            report(
              context,
              node,
              MSG.Iterator,
              HINT.SafeIterator,
            );
          },

          ForOfStatement(node) {
            if (node.right.type === "NewExpression") return;
            report(
              context,
              node.right,
              MSG.Iterator,
              HINT.SafeIterator,
            );
          },

          YieldExpression(node) {
            if (!node.delegate) return;
            if (node.argument?.type === "NewExpression") return;
            report(
              context,
              node,
              MSG.Iterator,
              HINT.SafeIterator,
            );
          },

          ArrayPattern(node) {
            const last = node.elements[node.elements.length - 1];
            const hasRest = last?.type === "RestElement";

            if (!hasRest) {
              report(
                context,
                node,
                MSG.Iterator,
                HINT.ObjectPattern,
              );
              return;
            }

            const parent = node.parent;
            if (parent?.type === "VariableDeclarator") {
              if (
                parent.init !== null &&
                parent.init.type !== "NewExpression"
              ) {
                report(
                  context,
                  parent,
                  MSG.Iterator,
                  HINT.SafeIterator,
                );
              }
            } else if (parent?.type === "AssignmentExpression") {
              if (parent.right.type !== "NewExpression") {
                report(
                  context,
                  parent,
                  MSG.Iterator,
                  HINT.SafeIterator,
                );
              }
            }
            // TODO(petamoriken): Support for deeply nested assignments
          },

          Literal(node) {
            if (!("regex" in node) || !node.regex) return;
            // Allowed when passed directly to `new SomeCtor(/re/)`
            if (node.parent?.type === "NewExpression") return;
            report(
              context,
              node,
              MSG.RegExp,
              HINT.SafeRegExp,
            );
          },

          BinaryExpression(node) {
            if (node.operator === "instanceof") {
              report(
                context,
                node,
                MSG.InstanceOf,
                HINT.InstanceOf,
              );
            } else if (
              node.operator === "in" &&
              // Private brand checks (`#brand in obj`) are the safe pattern
              // and must not use ReflectHas/ObjectHasOwn.
              node.left.type !== "PrivateIdentifier"
            ) {
              report(
                context,
                node,
                MSG.In,
                HINT.In,
              );
            }
          },
        };
      },
    },
  },
};

export default plugin;
