/**
 * Parser interface
 */
export type Parser = (html: string) => string;
export let parse: Parser = (_html) => {
  console.error("Error: deno-dom: No parser registered");
  Deno.exit(1);
};

export let parseFrag: Parser = (_html) => {
  console.error("Error: deno-dom: No parser registered");
  Deno.exit(1);
};

export function register(func: Parser, fragFunc: Parser) {
  parse = func;
  parseFrag = fragFunc;
}

