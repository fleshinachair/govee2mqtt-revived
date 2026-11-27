// The page's colours: Home Assistant's theme when shown inside it, otherwise
// the device's setting, unless light or dark was chosen. Loaded in <head>,
// before the page is drawn, so that it does not flash the wrong colours.
(() => {
  "use strict";

  // Specific, because inside Home Assistant the storage is shared with it
  const KEY = "govee2mqtt-theme";
  const root = document.documentElement;
  const prefersDark = window.matchMedia("(prefers-color-scheme: dark)");

  // Home Assistant shows this page in a frame on its own address, so its theme
  // can be read from the surrounding page. Opened directly, or framed by another
  // site, there is nothing to read.
  const homeAssistant = (() => {
    try {
      if (window.parent === window) {
        return null;
      }
      const html = window.parent.document.documentElement;
      const style = window.parent.getComputedStyle(html);
      return style.getPropertyValue("--primary-background-color") ? html : null;
    } catch {
      return null;
    }
  })();

  // Any CSS colour as [r, g, b], or null if it is not a solid colour
  const rgb = (value) => {
    const probe = document.createElement("span");
    probe.style.color = value.trim();
    if (!probe.style.color) {
      return null;
    }
    root.append(probe);
    const parts = getComputedStyle(probe).color.match(
      /^rgb\((\d+), (\d+), (\d+)\)$/,
    );
    probe.remove();
    return parts && parts.slice(1).map(Number);
  };

  const homeAssistantColors = () => {
    if (!homeAssistant) {
      return null;
    }
    const style = window.parent.getComputedStyle(homeAssistant);
    const background = rgb(
      style.getPropertyValue("--primary-background-color"),
    );
    const text = rgb(style.getPropertyValue("--primary-text-color"));
    return background && { background, text };
  };

  // Storage can be blocked, for example in a private window. The choice then
  // lasts only until the page is closed.
  const stored = () => {
    try {
      const value = localStorage.getItem(KEY);
      return value === "light" || value === "dark" ? value : "auto";
    } catch {
      return "auto";
    }
  };
  const store = (value) => {
    try {
      localStorage.setItem(KEY, value);
    } catch {}
  };

  let choice = stored();

  const setColor = (name, color) => {
    if (color) {
      root.style.setProperty(name, `rgb(${color.join(", ")})`);
    } else {
      root.style.removeProperty(name);
    }
  };

  const apply = () => {
    const colors = choice === "auto" ? homeAssistantColors() : null;
    let theme = choice;
    if (colors) {
      const [r, g, b] = colors.background;
      theme = 0.299 * r + 0.587 * g + 0.114 * b < 128 ? "dark" : "light";
    } else if (choice === "auto") {
      theme = prefersDark.matches ? "dark" : "light";
    }
    root.setAttribute("data-bs-theme", theme);
    setColor("--bs-body-bg", colors?.background);
    setColor("--bs-body-color", colors?.text);
  };

  apply();
  prefersDark.addEventListener("change", apply);

  if (homeAssistant) {
    // Home Assistant changes theme by restyling its root element
    const observer = new MutationObserver(apply);
    observer.observe(homeAssistant, {
      attributes: true,
      attributeFilter: ["style"],
    });
    window.addEventListener("pagehide", () => observer.disconnect());
  }

  window.addEventListener("DOMContentLoaded", () => {
    const picker = document.getElementById("theme");
    if (!picker) {
      return;
    }
    if (homeAssistant) {
      picker.querySelector('[value="auto"]').textContent =
        "Match Home Assistant";
    }
    picker.value = choice;
    picker.addEventListener("change", () => {
      choice = picker.value;
      store(choice);
      apply();
    });
  });
})();
