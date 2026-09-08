const leaf = `data:application/javascript,${
  encodeURIComponent('await import("http://localhost:4545/welcome.ts")')
}`;
const middle = `data:application/javascript,${
  encodeURIComponent(`await import(${JSON.stringify(leaf)})`)
}`;

await import(middle);
