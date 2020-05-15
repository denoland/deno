import { JSDOM } from './mod.ts';
import { assertEquals } from '../testing/asserts.ts';
import { runIfMain } from '../testing/bench.ts';

Deno.test('hello world', function(): void {
  let dom = new JSDOM("<!DOCTYPE html><p>Hello world</p>");
  let paragraph = dom.window.document.querySelector("p");
  assertEquals(paragraph.textContent, "Hello world");
});

Deno.test('run scripts', function (): void {
  let dom = new JSDOM(
    // eslint-disable-next-line max-len
    '<body><script>document.body.appendChild(document.createElement("hr"));</script></body>',
    { runScripts: "dangerously" }
  );
  let children = dom.window.document.body.children.length;
  assertEquals(children, 2);
});

Deno.test('serialize', function (): void {
  let dom = new JSDOM("<!DOCTYPE html>hello");
  assertEquals(
    dom.serialize(),
    "<!DOCTYPE html><html><head></head><body>hello</body></html>"
  );
  assertEquals(
    dom.window.document.documentElement.outerHTML,
    "<html><head></head><body>hello</body></html>"
  );
});

Deno.test('fragment', function (): void {
  let frag = JSDOM.fragment(`<p>Hello</p><p><strong>Hi!</strong>`);
  assertEquals(frag.childNodes.length, 2);
  assertEquals(frag.querySelector("strong").textContent, "Hi!");
});

runIfMain(import.meta);
