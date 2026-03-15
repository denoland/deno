// Block the event loop briefly to produce measurable delay
function blockEventLoop(ms: number) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    // busy wait
  }
}

// Block a few times to ensure delay samples are collected
blockEventLoop(50);
await new Promise((r) => setTimeout(r, 100));
blockEventLoop(50);
await new Promise((r) => setTimeout(r, 100));

// Keep alive long enough for at least one metric export
const timer = setTimeout(() => {}, 100000);
await new Promise((r) => setTimeout(r, 2000));
clearTimeout(timer);
