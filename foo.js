import express from "npm:express";

const app = express();
app.get("/", function (req, res) {
    res.send("Hello World");
});

app.listen(3000);
console.log("express app listening on http://localhost:3000");