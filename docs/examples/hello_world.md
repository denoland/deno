# Hello World

Deno is a secure runtime for both JavaScript and TypeScript. As the hello world
examples below highlight the same functionality can be created in JavaScript or
TypeScript, and Deno will execute both.

## JavaScript

In this JavaScript example the message `Hello [name]` is printed to the console
and the code ensures the name provided is capitalized.

**Command:** `deno run hello-world.js`

```js
function capitalize(word) {
  return word.charAt(0).toUpperCase() + word.slice(1);
}

function hello(name) {
  return "Hello " + capitalize(name);
}

console.log(hello("john"));
console.log(hello("Sarah"));
console.log(hello("kai"));

/**
 * Output:
 *
 * Hello John
 * Hello Sarah
 * Hello Kai
**/
```

## TypeScript

This TypeScript example is exactly the same as the JavaScript example above, the
code just has the additional type information which TypeScript supports.

The `deno run` command is exactly the same, it just references a `*.ts` file
rather than a `*.js` file.

**Command:** `deno run hello-world.ts`

```ts
function capitalize(word: string): string {
  return word.charAt(0).toUpperCase() + word.slice(1);
}

function hello(name: string): string {
  return "Hello " + capitalize(name);
}

console.log(hello("john"));
console.log(hello("Sarah"));
console.log(hello("kai"));

/**
 * Output:
 *
 * Hello John
 * Hello Sarah
 * Hello Kai
**/
```
