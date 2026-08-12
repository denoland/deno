// Verify that setTimeout/setInterval return NodeJS.Timeout
// and that NodeJS.Timeout is accepted by clearTimeout/clearInterval.
// This is the pattern used by many Node.js libraries.

const timeout: NodeJS.Timeout = setTimeout(() => {}, 1000);
clearTimeout(timeout);

const interval: NodeJS.Timeout = setInterval(() => {}, 1000);
clearInterval(interval);

// Also verify that passing a number still works
clearTimeout(123);
clearInterval(456);
