// const p = await fetch("http://localhost:8000/README.md");
// Download a large file from the internet
for (let i = 0; i < 1e4; i++) await fetch("http://localhost:8000/README.md");

gc();
