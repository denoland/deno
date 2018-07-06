// Remove and depend on @types/text-encoding once this PR is merged
// https://github.com/DefinitelyTyped/DefinitelyTyped/pull/26141
declare module "text-encoding" {
  export const TextEncoder: TextEncoder;
  export const TextDecoder: TextDecoder;
}
