## FAQs about TypeScript in Deno

### Can I use TypeScript not written for Deno?

Maybe. That is the best answer, we are afraid. For lots of reasons, Deno has
chosen to have fully qualified module specifiers. In part this is because it
treats TypeScript as a first class language. Also, Deno uses explicit module
resolution, with no _magic_. This is effectively the same way browsers
themselves work, thought they don't obviously support TypeScript directly. If
the TypeScript modules use imports that don't have these design decisions in
mind, they may not work under Deno.

Also, in recent versions of Deno (starting with 1.5), we have started to use a
Rust library to do transformations of TypeScript to JavaScript in certain
scenarios. Because of this, there are certain situations in TypeScript where
type information is required, and therefore those are not supported under Deno.
If you are using `tsc` as stand-alone, the setting to use is `"isolatedModules"`
and setting it to `true` to help ensure that your code can be properly handled
by Deno.

One of the ways to deal with the extension and the lack of _magical_ resolution
is to use

### What version(s) of TypeScript does Deno support?

Deno is built with a specific version of TypeScript. To find out what this is,
type the following on the command line:

```shell
> deno --version
```

The TypeScript version (along with the version of Deno and v8) will be printed.
Deno tries to keep up to date with general releases of TypeScript, providing
them in the next patch or minor release of Deno.

### There was a breaking change in the version of TypeScript that Deno uses, why did you break my program?

We do not consider changes in behavior or breaking changes in TypeScript
releases as breaking changes for Deno. TypeScript is a generally mature language
and breaking changes in TypeScript are almost always "good things" making code
more sound, and it is best that we all keep our code sound. If there is a
blocking change in the version of TypeScript and it isn't suitable to use an
older release of Deno until the problem can be resolved, then you should be able
to use `--no-check` to skip type checking all together.

In addition you can utilize `@ts-ignore` to _ignore_ a specific error in code
that you control. You can also replace whole dependencies, using
[import maps](../linking_to_external_code/import_maps), for situations where a
dependency of a dependency isn't being maintained or has some sort of breaking
change you want to bypass while waiting for it to be updated.
