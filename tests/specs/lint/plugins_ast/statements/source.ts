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
