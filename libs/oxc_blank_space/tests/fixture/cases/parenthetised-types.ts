var t = (): (void) => { }
//        ^^^^^^^^
var t1 = (): (void | string) => { }
//         ^^^^^^^^^^^^^^^^^
var t2 = (): void => { }
//         ^^^^^^
var t3 = (): void | number => { }
//         ^^^^^^^^^^^^^^^
type T = (void);
//^^^^^^^^^^^^^^ type alias
function f(): (void) { }
//          ^^^^^^^^
export {}
