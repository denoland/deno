// Copyright 2018-2025 the Deno authors. MIT license.

function setTheme(theme, themeToggle) {
  if (theme === "dark") {
    document.documentElement.classList.add("dark");
    document.documentElement.classList.remove("light");
    localStorage.setItem("deno-coverage-theme", "dark");
  } else {
    document.documentElement.classList.add("light");
    document.documentElement.classList.remove("dark");
    localStorage.setItem("deno-coverage-theme", "light");
  }

  const darkIcon = themeToggle.children[0];
  const lightIcon = themeToggle.children[1];

  if (theme === "dark") {
    darkIcon.style.display = "none";
    lightIcon.style.display = "block";
  } else {
    darkIcon.style.display = "block";
    lightIcon.style.display = "none";
  }
}

window.addEventListener("load", () => {
  const themeToggle = document.getElementById("theme-toggle");
  themeToggle.removeAttribute("style");

  const storedTheme = localStorage.getItem("deno-coverage-theme");
  const systemPrefersDark =
    window.matchMedia("(prefers-color-scheme: dark)").matches;

  if (storedTheme) {
    setTheme(storedTheme, themeToggle);
  } else {
    setTheme(systemPrefersDark ? "dark" : "light", themeToggle);
  }

  if (themeToggle) {
    themeToggle.addEventListener("click", () => {
      const isDark = document.documentElement.classList.contains("dark");
      setTheme(isDark ? "light" : "dark", themeToggle);
    });
  }
});

// prevent flash
const theme = localStorage.getItem("deno-coverage-theme") ||
  (window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light");
document.documentElement.classList.add(theme);
