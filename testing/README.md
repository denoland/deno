# Testing 

## Usage

```ts
import { test, assert, equal, assertEqual } from 'https://deno.land/x/testing/mod.ts';

test({
  name: 'testing example',
  fn() {
    assert(equal("world", "world"));
    assert(!equal("hello", "world"));
    assert(equal({ hello: "world" }, { hello: "world" }));
    assert(!equal({ world: "hello" }, { hello: "world" }));
    assertEqual("world", "world");
    assertEqual({hello: "world"}, {hello: "world"});
  },
});
```

Short syntax (named function instead of object):
```ts
test(function example() {
  assert(equal("world", "world"));
  assert(!equal("hello", "world"));
  assert(equal({ hello: "world" }, { hello: "world" }));
  assert(!equal({ world: "hello" }, { hello: "world" }));
  assertEqual("world", "world");
  assertEqual({hello: "world"}, {hello: "world"});
});
```
