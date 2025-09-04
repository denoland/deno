export async function randomTimeout(min, max) {
  const timeout = Math.floor(Math.random() * (max - min + 1) + min);
  return new Promise((resolve) => setTimeout(resolve, timeout));
}
