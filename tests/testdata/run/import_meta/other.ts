console.log(
  import.meta.url.startsWith("http") ? "other remote" : "other",
  import.meta.url,
  import.meta.main,
  import.meta.filename,
  import.meta.dirname,
);
