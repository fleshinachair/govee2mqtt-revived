// Light, dark, or whatever the device is set to. Loaded in <head>, before the
// page is drawn, so that it does not flash the wrong colours.
(() => {
  "use strict";

  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)");

  // Storage can be blocked, for example in a private window. The choice then
  // lasts only until the page is closed.
  const stored = () => {
    try {
      const value = localStorage.getItem("theme");
      return value === "light" || value === "dark" ? value : "auto";
    } catch {
      return "auto";
    }
  };
  const store = (value) => {
    try {
      localStorage.setItem("theme", value);
    } catch {}
  };

  let choice = stored();

  const apply = () => {
    const theme =
      choice === "auto" ? (prefersDark.matches ? "dark" : "light") : choice;
    document.documentElement.setAttribute("data-bs-theme", theme);
  };

  apply();
  prefersDark.addEventListener("change", apply);

  window.addEventListener("DOMContentLoaded", () => {
    const picker = document.getElementById("theme");
    if (!picker) {
      return;
    }
    picker.value = choice;
    picker.addEventListener("change", () => {
      choice = picker.value;
      store(choice);
      apply();
    });
  });
})();
