# Strings

This module provides a few basic utilities to manipulate strings.

## Usage

### pad

Input string is processed to output a string with a minimal length. If the
parameter `strict` is set to true, the output string length is equal to the
`strLen` parameter.

Basic usage:

```ts
import { pad } from "https://deno.land/std/strings/pad.ts";
pad("deno", 6, { char: "*", side: "left" }); // output : "**deno"
pad("deno", 6, { char: "*", side: "right" }); // output : "deno**"
pad("denosorusrex", 6, {
  char: "*",
  side: "left",
  strict: true,
  strictSide: "right",
  strictChar: "...",
}); // output : "den..."
```
