import {  test, assertEquals } from "./test_util.ts";
import { runIfMain } from "./deps/https/deno.land/std/testing/mod.ts";
import { h, renderToString } from "./jsx.ts";
import { Layout } from "./jsx_view.tsx";

test(function jsxH(): void {
  const v = h(() => <a href="a">bbb</a>);
  assertEquals(typeof v.type, "function");
  assertEquals(v.props, undefined);
  assertEquals(v.children, []);
});

test(function jsxH2(): void {
  const v = h("div", {class: "deno"}, "land");
  assertEquals(v.type, "div");
  assertEquals(v.props, {class: "deno"});
  assertEquals(v.children, ["land"]);
});

test(function jsxBasic(): void {
  const str = renderToString(<a href="https://deno.land" class="clz">link</a>);
  const exp = `<a href="https://deno.land" class="clz" >link</a>`;
  assertEquals(str, exp);
});

test(function jsxImported(): void {
  const str = renderToString(<Layout title={"deno"}>land</Layout>);
  const exp = `<html><head><title>deno</title></head><body>land</body></html>`;
  assertEquals(str, exp);
});

test(function jsxRendrerToStringIgnoredProps(): void {
  const v = h("div", {
    str: "str",
    num: 1,
    bool: true,
    nul: null,
    undef: undefined,
    symbol: Symbol("a"),
    inst: new TextEncoder(),
    bigint: BigInt(11),
    obj: {},
    arr: [],
    func: () => { },
  }, "land");
  // @ts-ignore
  const str = renderToString(v);
  assertEquals(str, `<div str="str" num="1" bool="true" nul="null" >land</div>`);
});


runIfMain(import.meta);
