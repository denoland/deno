/**
 * Creates a tracing field for use with the `@instrument` decorator.
 *
 * @example Instrument a method with a custom field
 * ```ts
 * import { instrument, field } from "@bcheidemann/tracing";
 *
 * class Example {
 *   @instrument(field("fieldName", "fieldValue"))
 *   test() {
 *     // ...
 *   }
 * }
 * ```
 *
 * @example Instrument a function with a custom field
 * ```ts
 * import { instrumentCallback, field } from "@bcheidemann/tracing";
 *
 * const test = instrumentCallback(
 *   [field("fieldName", "fieldValue")],
 *   function test() {
 *     // ...
 *   }
 * );
 * ```
 *
 * @param name The name of the field
 * @param value The value of the field
 */
export function field(name: string, value: unknown): unknown {
  return { name, value };
}
