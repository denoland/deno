let i = 0;

while (true) {
  doSomething(10000);
  i++;
}

function doSomething(breakDuration) {
  console.log("hello %i", i);
  console.timeStamp();

  const started = performance.now();
  let canBreak = false;

  while (!canBreak) {
    canBreak = performance.now() - started >= breakDuration;
  }
}
