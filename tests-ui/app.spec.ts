import { test, expect } from "@playwright/test";

test("browser preview makes the desktop requirement clear", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Proxy checker" }),
  ).toBeVisible();
  await expect(
    page.getByText("This is a browser preview.", { exact: false }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Check all", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Copy failed", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Add your first proxies" }),
  ).toBeDisabled();
  expect(errors).toEqual([]);
  await page.screenshot({
    path: "artifacts/browser-empty.png",
    fullPage: true,
  });
});

test("help is keyboard accessible and explains protocol semantics", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Formats & help" }).click();
  const dialog = page.getByRole("dialog", { name: "Formats & help" });
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByText("means TLS to the proxy itself", { exact: false }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
});

test("layout remains within the minimum desktop width", async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 650 });
  await page.goto("/");
  const widths = await page.evaluate(() => ({
    viewport: innerWidth,
    content: document.documentElement.scrollWidth,
  }));
  expect(widths.content).toBeLessThanOrEqual(widths.viewport);
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  await expect(
    page.getByRole("dialog", { name: "Check settings" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Save settings" }),
  ).toBeVisible();
});

test("backup controls explain file contents and fit the minimum window", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1000, height: 650 });
  await page.goto("/");
  await page
    .getByRole("button", { name: "Backup & restore", exact: true })
    .click();
  const dialog = page.getByRole("dialog", { name: "Backup & restore" });
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByText("without encryption", { exact: false }),
  ).toBeVisible();
  await dialog.getByLabel("Include in backup").selectOption("settings");
  await expect(dialog.getByLabel("Include in backup")).toHaveValue("settings");
  await expect(
    dialog.getByRole("button", { name: "Export backup", exact: true }),
  ).toBeDisabled();
  await expect(
    dialog.getByRole("button", { name: "Import backup", exact: true }),
  ).toBeDisabled();
  await expect(
    dialog.getByRole("button", { name: "Close", exact: true }),
  ).toBeVisible();
  const dimensions = await dialog.evaluate((el) => ({
    width: el.scrollWidth,
    visible: el.clientWidth,
    bottom: el.getBoundingClientRect().bottom,
  }));
  expect(dimensions.width).toBeLessThanOrEqual(dimensions.visible);
  expect(dimensions.bottom).toBeLessThanOrEqual(650);
  await page.keyboard.press("Escape");
  await expect(dialog).not.toBeVisible();
});
