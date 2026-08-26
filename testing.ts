// main.ts — an ordinary Express app, unchanged
import express from "npm:express";

const app = express();
app.get("/", (_req, res) => res.send("<h1>Hello, desktop 👋</h1>"));

app.listen(); // ← the desktop runtime picks the port
