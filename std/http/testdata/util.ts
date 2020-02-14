export function relativeResolver(m: ImportMeta): (path: string) => string {
  return (s: string): string => new URL(s, m.url).pathname;
}
