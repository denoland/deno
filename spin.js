const swirl = [
  "⢀⠀",
  "⡀⠀",
  "⠄⠀",
  "⢂⠀",
  "⡂⠀",
  "⠅⠀",
  "⢃⠀",
  "⡃⠀",
  "⠍⠀",
  "⢋⠀",
  "⡋⠀",
  "⠍⠁",
  "⢋⠁",
  "⡋⠁",
  "⠍⠉",
  "⠋⠉",
  "⠋⠉",
  "⠉⠙",
  "⠉⠙",
  "⠉⠩",
  "⠈⢙",
  "⠈⡙",
  "⢈⠩",
  "⡀⢙",
  "⠄⡙",
  "⢂⠩",
  "⡂⢘",
  "⠅⡘",
  "⢃⠨",
  "⡃⢐",
  "⠍⡐",
  "⢋⠠",
  "⡋⢀",
  "⠍⡁",
  "⢋⠁",
  "⡋⠁",
  "⠍⠉",
  "⠋⠉",
  "⠋⠉",
  "⠉⠙",
  "⠉⠙",
  "⠉⠩",
  "⠈⢙",
  "⠈⡙",
  "⠈⠩",
  "⠀⢙",
  "⠀⡙",
  "⠀⠩",
  "⠀⢘",
  "⠀⡘",
  "⠀⠨",
  "⠀⢐",
  "⠀⡐",
  "⠀⠠",
  "⠀⢀",
  "⠀⡀",
];
let frame = 0;

function spinnerFrame() {
  const len = swirl.length;
  let output = "";

  const idx = frame % len;

  // Make the "head" bold for visibility
  output += `\x1b[1m${swirl[idx]}\x1b[0m`; // bold head

  process.stdout.write("\r" + output.padEnd(30, " "));
  frame = (frame + 1) % len;
}

const interval = setInterval(spinnerFrame, 80);

// Stop after 15 seconds
setTimeout(() => {
  clearInterval(interval);
  process.stdout.write("\rDone!                      \n");
}, 15000);
