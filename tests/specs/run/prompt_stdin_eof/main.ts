// Test that prompt() does not output extra newline when stdin is closed
// Issue: https://github.com/denoland/deno/issues/22956

const name = prompt("What is your name?", "Jane");
console.log(`Name: ${name}`);

const input = prompt("Enter something:");
console.log(`Input: ${input}`);
