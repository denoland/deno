// Break
while (false) {
  break;
}

outer: while (false) {
  break outer;
}

// Continue
while (false) {
  continue;
}

outer: while (false) {
  continue outer;
}

// Debugger
debugger;

// Return
(() => {
  return;
});
(() => {
  return 1;
});

// For loops
for (const a in b) {
  foo;
}
for (a in b) foo;
for (const a of b) foo;
for (const [a, b] of c) foo;
for (const { a, b } of c) foo;
for await (const a of b) foo;
for (let i = 0; i < 10; i++) {
  foo;
}

switch (foo) {
  case 1:
  case 2:
    break;
  default:
    foo;
}

try {
} catch (e) {
} finally {
}

try {
} finally {
}

try {
} catch {
}

try {
} catch (e) {
}

if (false) {
  1;
}

if (false) {
  1;
} else {
  2;
}
