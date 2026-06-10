// Copyright 2018-2026 the Deno authors. MIT license.
import { assertEquals, assertThrows } from "@std/assert";
import vm, {
  compileFunction,
  createContext,
  isContext,
  runInContext,
  runInNewContext,
  runInThisContext,
  Script,
  SourceTextModule,
  SyntheticModule,
} from "node:vm";

Deno.test({
  name: "vm runInNewContext",
  fn() {
    const two = runInNewContext("1 + 1");
    assertEquals(two, 2);
  },
});

Deno.test({
  name: "vm new Script()",
  fn() {
    const script = new Script(`
function add(a, b) {
  return a + b;
}
const x = add(1, 2);
x
`);

    const value = script.runInThisContext();
    assertEquals(value, 3);
  },
});

// https://github.com/denoland/deno/issues/23186
Deno.test({
  name: "vm runInNewContext sandbox",
  fn() {
    const sandbox = { fromAnotherRealm: false };
    runInNewContext("fromAnotherRealm = {}", sandbox);

    assertEquals(typeof sandbox.fromAnotherRealm, "object");
  },
});

// https://github.com/denoland/deno/issues/22395
Deno.test({
  name: "vm runInewContext with context object",
  fn() {
    const context = { a: 1, b: 2 };
    const result = runInNewContext("a + b", context);
    assertEquals(result, 3);
  },
});

// https://github.com/denoland/deno/issues/18299
Deno.test({
  name: "vm createContext and runInContext",
  fn() {
    // @ts-expect-error implicit any
    globalThis.globalVar = 3;

    const context = { globalVar: 1 };
    createContext(context);
    runInContext("globalVar *= 2", context);
    assertEquals(context.globalVar, 2);
    // @ts-expect-error implicit any
    assertEquals(globalThis.globalVar, 3);
  },
});

Deno.test({
  name: "vm runInThisContext Error rethrow",
  fn() {
    assertThrows(
      () => {
        runInThisContext("throw new Error('error')");
      },
      Error,
      "error",
    );
    assertThrows(
      () => {
        runInThisContext("throw new TypeError('type error')");
      },
      TypeError,
      "type error",
    );
  },
});

// https://github.com/webpack/webpack/blob/87660921808566ef3b8796f8df61bd79fc026108/lib/javascript/JavascriptParser.js#L4329
Deno.test({
  name: "vm runInNewContext webpack magic comments",
  fn() {
    const webpackCommentRegExp = new RegExp(
      /(^|\W)webpack[A-Z]{1,}[A-Za-z]{1,}:/,
    );
    const comments = [
      'webpackChunkName: "test"',
      'webpackMode: "lazy"',
      "webpackPrefetch: true",
      "webpackPreload: true",
      "webpackProvidedExports: true",
      'webpackChunkLoading: "require"',
      'webpackExports: ["default", "named"]',
    ];

    for (const comment of comments) {
      const result = webpackCommentRegExp.test(comment);
      assertEquals(result, true);

      const [[key, _value]]: [string, string][] = Object.entries(
        runInNewContext(`(function(){return {${comment}};})()`),
      );
      const expectedKey = comment.split(":")[0].trim();
      assertEquals(key, expectedKey);
    }
  },
});

// https://github.com/denoland/deno/issues/18315
Deno.test({
  name: "vm isContext",
  fn() {
    // Currently we do not expose VM contexts so this is always false.
    const obj = {};
    assertEquals(isContext(obj), false);
    assertEquals(isContext(globalThis), false);
    const sandbox = runInNewContext("({})");
    assertEquals(isContext(sandbox), false);
  },
});

// https://github.com/denoland/deno/issues/23297
Deno.test({
  name: "vm context promise rejection",
  fn() {
    const code = `
function reject() {
  return Promise.reject(new Error('rejected'));
}
reject().catch(() => {})
    `;

    const script = new Script(code);
    script.runInNewContext();
  },
});

// https://github.com/denoland/deno/issues/22441
// https://github.com/denoland/deno/issues/33385
// `import()` inside a `vm.Script` that was compiled without an
// `importModuleDynamically` callback must reject with
// `ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING`, matching Node — otherwise a
// sandboxed script could escape via dynamic import.
Deno.test({
  name: "vm runInNewContext module loader",
  async fn() {
    const code =
      `globalThis.__p = import('node:process').then((m) => ({ ok: true, m }), (e) => ({ ok: false, code: e.code, name: e.name, message: e.message }));`;
    const script = new Script(code);
    const sandbox: Record<string, unknown> = {};
    script.runInNewContext(sandbox);
    const result = await (sandbox.__p as Promise<{
      ok: boolean;
      code?: string;
      name?: string;
      message?: string;
    }>);
    assertEquals(result.ok, false);
    assertEquals(result.code, "ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING");
    assertEquals(result.name, "TypeError");
    assertEquals(
      result.message,
      "A dynamic import callback was not specified.",
    );
  },
});

function dynamicImportModule(value: string) {
  const module = new SyntheticModule(["value"], () => {
    module.setExport("value", value);
  });
  return module;
}

function referrerName(referrer: unknown) {
  return (referrer as { constructor?: { name?: string } } | undefined)
    ?.constructor?.name;
}

Deno.test({
  name: "vm importModuleDynamically callback resolves modules",
  async fn() {
    const seen: string[] = [];
    const callback = (
      specifier: string,
      referrer: unknown,
      attributes: Record<string, string | undefined>,
    ) => {
      seen.push(
        `${specifier}:${referrerName(referrer)}:${attributes.type}`,
      );
      return dynamicImportModule(`${specifier}:${attributes.type}`);
    };
    const code =
      "globalThis.__p = import('vm:script', { with: { type: 'json' } }).then((m) => m.value);";

    delete (globalThis as typeof globalThis & { __p?: Promise<string> }).__p;
    new Script(code, { importModuleDynamically: callback })
      .runInThisContext();
    assertEquals(
      await (globalThis as typeof globalThis & { __p: Promise<string> }).__p,
      "vm:script:json",
    );

    const context = createContext({ Promise });
    runInContext(
      "globalThis.p = import('vm:context').then((m) => m.value);",
      context,
      { importModuleDynamically: callback },
    );
    assertEquals(await context.p, "vm:context:undefined");

    const sandbox: Record<string, unknown> = { Promise };
    runInNewContext(
      "globalThis.p = import('vm:new-context').then((m) => m.value);",
      sandbox,
      { importModuleDynamically: callback },
    );
    assertEquals(await sandbox.p, "vm:new-context:undefined");

    assertEquals(seen, [
      "vm:script:Script:json",
      "vm:context:Script:undefined",
      "vm:new-context:Script:undefined",
    ]);
  },
});

Deno.test({
  name: "vm importModuleDynamically callback works for compileFunction",
  async fn() {
    let referrer: unknown;
    const fn = compileFunction("return import('vm:fn');", [], {
      importModuleDynamically(_specifier, ref) {
        referrer = ref;
        return Promise.resolve(dynamicImportModule("from function"));
      },
    });
    const ns = await fn();
    assertEquals(ns.value, "from function");
    assertEquals(referrer, fn);
  },
});

Deno.test({
  name: "vm createContext importModuleDynamically callback is inherited",
  async fn() {
    const context = createContext({ Promise }, {
      importModuleDynamically(specifier) {
        return dynamicImportModule(`context default ${specifier}`);
      },
    });
    runInContext(
      "globalThis.p = import('vm:ctx-default').then((m) => m.value);",
      context,
    );
    assertEquals(await context.p, "context default vm:ctx-default");
  },
});

Deno.test({
  name: "vm SourceTextModule importModuleDynamically callback resolves modules",
  async fn() {
    let referrer: unknown;
    const root = new SourceTextModule(
      "globalThis.__stm = import('vm:stm').then((m) => m.value);",
      {
        importModuleDynamically(_specifier: string, ref: unknown) {
          referrer = ref;
          return dynamicImportModule("from source text module");
        },
      },
    );
    await root.link(() => {
      throw new Error("unexpected static import");
    });
    await root.evaluate();
    assertEquals(
      await (globalThis as typeof globalThis & { __stm: Promise<string> })
        .__stm,
      "from source text module",
    );
    assertEquals(referrer, root);
  },
});

// https://github.com/denoland/deno/issues/31783
Deno.test({
  name: "vm SourceTextModule default import evaluates namespace",
  async fn() {
    const context = vm.createContext({});
    const module = new vm.SourceTextModule(
      "export const answer = 42;",
      { context },
    );

    await module.link(() => {
      throw new Error("unexpected static import");
    });
    await module.evaluate();

    assertEquals((module.namespace as { answer: number }).answer, 42);
  },
});

// https://github.com/denoland/deno/issues/23913
Deno.test({
  name: "vm memory leak crash",
  fn() {
    const script = new Script("returnValue = 2+2");

    for (let i = 0; i < 1000; i++) {
      script.runInNewContext({}, { timeout: 10000 });
    }
  },
});

// https://github.com/denoland/deno/issues/23852
Deno.test({
  name: "vm runInThisContext global.foo",
  fn() {
    const result = runInThisContext(`global.foo = 1`);
    assertEquals(result, 1);
  },
});

// https://github.com/denoland/deno/issues/34185
Deno.test({
  name:
    "vm createContext/runInContext in a tight loop does not panic at teardown",
  fn() {
    // Each iteration creates a new contextified sandbox, compiles a script
    // and runs it. Previously the per-context `Weak<Context>` registered by
    // the v8 crate's `Context::set_slot` would survive into isolate teardown
    // and panic in the final GC's first-pass weak callback because the
    // backing `WeakData` had already been freed by `dispose_annex` without
    // resetting the v8 Global. We need enough iterations to trigger the
    // teardown GC's final weak-callback pass; 10000 is more than sufficient.
    for (let i = 0; i < 10000; i++) {
      const sandbox: Record<string, unknown> = {};
      createContext(sandbox);
      const s = new Script("x = 42");
      s.runInContext(sandbox);
      assertEquals(sandbox.x, 42);
    }
  },
});

// https://github.com/denoland/deno/issues/32921
Deno.test({
  name: "vm in operator walks prototype chain of sandbox",
  fn() {
    class EventTarget {
      addEventListener() {}
    }

    const windowPrototype = Object.create(EventTarget.prototype);

    // deno-lint-ignore no-explicit-any
    function Window(this: any) {
      createContext(this);
      this._globalProxy = runInContext("this", this);
      Object.setPrototypeOf(this, windowPrototype);
      // deno-lint-ignore no-this-alias
      const window = this;
      Object.defineProperty(this, "window", {
        get() {
          return window._globalProxy;
        },
        enumerable: true,
        configurable: true,
      });
    }

    const window =
      new (Window as unknown as new () => Record<string, unknown>)();

    // Proto-chain hit: addEventListener lives on EventTarget.prototype
    assertEquals(runInContext(`"addEventListener" in window`, window), true);
    // Own property: "window" is defined directly on the sandbox
    assertEquals(runInContext(`"window" in window`, window), true);
    // Negative case: property not on the chain
    assertEquals(runInContext(`"doesNotExist" in window`, window), false);
  },
});
