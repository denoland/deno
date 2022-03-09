const { createCanvas } = require("@napi-rs/canvas");

const canvas = createCanvas(300, 320);
const ctx = canvas.getContext("2d");

// Do some random stuff, make sure it works.
ctx.lineWidth = 10;
ctx.strokeStyle = "#03a9f4";
ctx.fillStyle = "#03a9f4";
ctx.strokeRect(75, 140, 150, 110);
ctx.fillRect(130, 190, 40, 60);
ctx.beginPath();
ctx.moveTo(50, 140);
ctx.lineTo(150, 60);
ctx.lineTo(250, 140);
ctx.closePath();
ctx.stroke();

canvas.encode("png").then(console.log);
