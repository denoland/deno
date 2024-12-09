Ensures that the code is fully written in ASCII characters.

V8, the JavaScript engine Deno relies on, provides a method that strings get
populated outside V8's heap. In particular, if they are composed of one-byte
characters only, V8 can handle them much more efficiently through
[`v8::String::ExternalOneByteStringResource`]. In order to leverage this V8
feature in the internal of Deno, this rule checks if all characters in the code
are ASCII.

[`v8::String::ExternalOneByteStringResource`]: https://v8.github.io/api/head/classv8_1_1String_1_1ExternalOneByteStringResource.html

That said, you can also make use of this lint rule for something other than
Deno's internal JavaScript code. If you want to make sure your codebase is made
up of ASCII characters only (e.g. want to disallow non-ASCII identifiers) for
some reasons, then this rule will be helpful.

### Invalid:

```typescript
const π = Math.PI;

// string literals are also checked
const ninja = "🥷";

function こんにちは(名前: string) {
  console.log(`こんにちは、${名前}さん`);
}

// “comments” are also checked
// ^        ^
// |        U+201D
// U+201C
```

### Valid:

```typescript
const pi = Math.PI;

const ninja = "ninja";

function hello(name: string) {
  console.log(`Hello, ${name}`);
}

// "comments" are also checked
```
