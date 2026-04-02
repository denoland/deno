
// Simple case
const a = async<T>(v: T) => {};
//             ^^^  ^^^

// Hard case - generic spans multiple lines
const b = async <
    T
>/**/(/**/v: T) => {};
//   ^     ^^^

// Harder case - generic and return type spans multiple lines
const c = async <
    T
>(v: T): Promise<
// ^^^ ^^^^^^^^^^
    T
> => v;

// https://github.com/bloomberg/ts-blank-space/issues/29
(function () {
    return<T>
        (v: T) => v
});
(function () {
    return/**/<
        T
    >/**/(v: T)/**/:
    T/**/=> v
});
(function* () {
    yield<T>
(v: T)=>v;
});
(function* () {
    throw<T>
(v: T)=>v;
});
