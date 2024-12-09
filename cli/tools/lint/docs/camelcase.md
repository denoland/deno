Enforces the use of camelCase in variable names

Consistency in a code base is key for readability and maintainability. This rule
enforces variable declarations and object property names which you create to be
in camelCase.

Of note:

- `_` is allowed at the start or end of a variable
- All uppercase variable names (e.g. constants) may have `_` in their name
- If you have to use a snake_case key in an object for some reasons, wrap it in
  quotation mark
- This rule also applies to variables imported or exported via ES modules, but
  not to object properties of those variables

### Invalid:

```typescript
let first_name = "Ichigo";
const obj1 = { last_name: "Hoshimiya" };
const obj2 = { first_name };
const { last_name } = obj1;

function do_something() {}
function foo({ snake_case = "default value" }) {}

class snake_case_class {}
class Also_Not_Valid_Class {}

import { not_camelCased } from "external-module.js";
export * as not_camelCased from "mod.ts";

enum snake_case_enum {
  snake_case_variant,
}

type snake_case_type = { some_property: number };

interface snake_case_interface {
  some_property: number;
}
```

### Valid:

```typescript
let firstName = "Ichigo";
const FIRST_NAME = "Ichigo";
const __myPrivateVariable = "Hoshimiya";
const myPrivateVariable_ = "Hoshimiya";
const obj1 = { "last_name": "Hoshimiya" }; // if an object key is wrapped in quotation mark, then it's valid
const obj2 = { "first_name": first_name };
const { last_name: lastName } = obj;

function doSomething() {} // function declarations must be camelCase but...
do_something(); // ...snake_case function calls are allowed
function foo({ snake_case: camelCase = "default value" }) {}

class PascalCaseClass {}

import { not_camelCased as camelCased } from "external-module.js";
export * as camelCased from "mod.ts";

enum PascalCaseEnum {
  PascalCaseVariant,
}

type PascalCaseType = { someProperty: number };

interface PascalCaseInterface {
  someProperty: number;
}
```
