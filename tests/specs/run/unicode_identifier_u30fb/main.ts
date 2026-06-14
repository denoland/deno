// U+30FB KATAKANA MIDDLE DOT is valid in identifiers since Unicode 15.1.
// https://github.com/denoland/deno/issues/35145
const あ・あ = 1;
console.log(あ・あ);

const obj = { あ・あ: 2 } as const;
console.log(obj.あ・あ);
