const name0 = prompt("What is your name?", "Jane Doe"); // Answer John Doe
console.log(`Your name is ${name0}.`);
const name1 = prompt("What is your name?", "Jane Doe"); // Answer with default
console.log(`Your name is ${name1}.`);
const input = prompt(); // Answer foo
console.log(`Your input is ${input}.`);
const answer0 = confirm("Question 0"); // Answer y
console.log(`Your answer is ${answer0}`);
const answer1 = confirm("Question 1"); // Answer n
console.log(`Your answer is ${answer1}`);
const answer2 = confirm("Question 2"); // Answer with yes (returns false)
console.log(`Your answer is ${answer2}`);
const answer3 = confirm(); // Answer with default
console.log(`Your answer is ${answer3}`);
const windows = prompt("What is Windows EOL?");
console.log(`Your answer is ${JSON.stringify(windows)}`);
alert("Hi");
alert();
console.log("The end of test");
const eof = prompt("What is EOF?");
console.log(`Your answer is ${JSON.stringify(eof)}`);
