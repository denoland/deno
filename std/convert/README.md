# std/convert

Modules to convert from one format of syntax to another.

## std/convert/markdown

Based on [marked](https://github.com/markedjs/marked) is a powerful and fast,
markdown parser.

Basic usage uses the `parse()` function exported from the `markdown.ts`, which
takes in a markdown string and returns an HTML string. By default it supports
Git Flavored Markdown (GFM).

A basic example:

```ts
import { parse } from "https://deno.land/std/convert/marked.ts";

const md = `# Hello markdown

Some basic markdown.
`;

const html = parse(md);
```

**WARNING** This library does not sanitize raw HTML in your markdown. If you
cannot trust the raw HTML in your source markdown, you need to utilize a library
to sanitize your HTML.
