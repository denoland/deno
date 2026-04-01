
namespace Empty {}
// ^^^^^^^^^^^^^^^ empty namespace

namespace TypeOnly {
    type A = string;

    export type B = A | number;

    export interface I {}

    export namespace Inner {
        export type C = B;
    }
}
// ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ type-only namespace

namespace My.Internal.Types {
    export type Foo = number;
}

namespace With.Imports {
    import Types = My.Internal.Types;
    export type Foo = Types.Foo;
}
// ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ nested namespaces

// declaring the existence of a runtime namespace:
declare namespace Declared {
    export function foo(): void
}
// ^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `declare namespace`

export namespace ValueImport {
    import foo = Declared.foo;
    export type T = typeof foo;
}
// ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ // _value_ import namespace

Declared.foo(); // May throw at runtime if declaration was false

export const x: With.Imports.Foo = 1;
//            ^^^^^^^^^^^^^^^^^^
