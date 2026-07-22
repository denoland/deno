// Copyright 2018-2026 the Deno authors. MIT license.

const STORAGE_KEY = "deno-coverage-theme";

function getStoredTheme() {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

function setStoredTheme(theme) {
  try {
    localStorage.setItem(STORAGE_KEY, theme);
  } catch {
    // Ignore storage failures so the report remains usable in restricted pages.
  }
}

function getSystemTheme() {
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function setTheme(theme, themeToggle) {
  if (theme === "dark") {
    document.documentElement.classList.add("dark");
    document.documentElement.classList.remove("light");
    setStoredTheme("dark");
  } else {
    document.documentElement.classList.add("light");
    document.documentElement.classList.remove("dark");
    setStoredTheme("light");
  }

  if (!themeToggle) {
    return;
  }

  const darkIcon = themeToggle.children[0];
  const lightIcon = themeToggle.children[1];

  if (!darkIcon || !lightIcon) {
    return;
  }

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
  if (!themeToggle) {
    return;
  }

  themeToggle.removeAttribute("style");

  setTheme(getStoredTheme() || getSystemTheme(), themeToggle);

  themeToggle.addEventListener("click", () => {
    const isDark = document.documentElement.classList.contains("dark");
    setTheme(isDark ? "light" : "dark", themeToggle);
  });
});

// prevent flash
document.documentElement.classList.add(getStoredTheme() || getSystemTheme());
