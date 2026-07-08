/**
 * The line range of the extracted doc test lives in the URL fragment
 * (`mod.ts#5-12.ts`), so the path of `import.meta.url` stays equal to the
 * original source file and can be read back from disk (#29684).
 *
 * ```ts
 * const path = import.meta.filename!;
 * const content = await Deno.readTextFile(path);
 * console.log(path.endsWith("mod.ts"));
 * console.log(content.includes("export function add"));
 * ```
 */
export function add(a: number, b: number): number {
  return a + b;
}
