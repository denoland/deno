const answer = prompt("Cancel me", "default");
console.log(`answer=${answer === null ? "null" : JSON.stringify(answer)}`);
console.log("after prompt");
