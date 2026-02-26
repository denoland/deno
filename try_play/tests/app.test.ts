import { expect, test } from "@playwright/test";

test("page has correct title", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle("Test App");
});

test("greeting is visible", async ({ page }) => {
  await page.goto("/");
  const heading = page.locator("#greeting");
  await expect(heading).toHaveText("Hello, Playwright!");
});

test("counter button works", async ({ page }) => {
  await page.goto("/");
  const button = page.locator("#counter-btn");
  await expect(button).toHaveText("Clicked 0 times");
  await button.click();
  await expect(button).toHaveText("Clicked 1 times");
  await button.click();
  await expect(button).toHaveText("Clicked 2 times");
});
