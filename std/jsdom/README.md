# jsdom

Port of [jsdom](https://github.com/jsdom/jsdom).

> jsdom is a pure-JavaScript implementation of many web standards, notably the WHATWG [DOM](https://dom.spec.whatwg.org/) and [HTML](https://html.spec.whatwg.org/multipage/) Standards, for use with ~~Node.js~~ Deno. In general, the goal of the project is to emulate enough of a subset of a web browser to be useful for testing and scraping real-world web applications.

## Usage

```typescript
import { JSDOM } from "https://deno.land/std/jsdom/mod.ts";

let dom = new JSDOM(`
  <!DOCTYPE html>
  <p>Hello, Deno!</p>
`);
let p = dom.window.document.querySelector("p");
console.log(p.textContent); // => "Hello, Deno!"
```
