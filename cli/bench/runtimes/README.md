# Bench suite to compare Deno, Bun an Node.js

Inspired by benchmarks used on https://bun.sh. 

Some of the code in this directory comes from https://github.com/oven-sh/bun/tree/main/bench.

Start with:

```shellsession
$ deno task setup
```


Self-contained benchmarks

```shellsession
$ deno task ffi:bun
$ deno task ffi:deno
$ deno task sqlite:bun
$ deno task sqlite:deno
$ deno task sqlite:node
```

HTTP benchmarks (bench against `wrk`)

```shellsession
$ deno task ssr:bun
$ deno task ssr:deno
$ deno task ssr:node
```