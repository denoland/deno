/**/import type T from "node:assert";
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `import type`

import { "🙂" as C2 } from "./modules";

type I = any;
class C {}
C === C2;

/**/export type { I };
//  ^^^^^^^^^^^^^^^^^^ `export type`

/**/export type * from "node:buffer";
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ `export type *`

import {type AssertPredicate/**/, deepEqual} from "node:assert";
//      ^^^^^^^^^^^^^^^^^^^^^^^^^

export {
    C,
    type T,
//  ^^^^^^
    C as "🙂"
}

/**/export type T2 = 1;
//  ^^^^^^^^^^^^^^^^^^^

export default {
    v: true as false
//         ^^^^^^^^^
};
