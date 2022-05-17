for (let i = 0; i < 100; i++) {
  const response = await fetch("http://localhost:8000/README.md");
  await response.text();
}
