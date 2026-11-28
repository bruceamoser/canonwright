import { useCallback, useEffect, useState } from "react";

export type ThemeChoice = "system" | "light" | "dark";
export type ResolvedTheme = "light" | "dark";

const STORAGE_KEY = "canonwright.theme";

function systemTheme(): ResolvedTheme {
  return window.matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

export function storedChoice(): ThemeChoice {
  const stored = window.localStorage.getItem(STORAGE_KEY);
  return stored === "light" || stored === "dark" || stored === "system" ? stored : "light";
}

/** Paint the theme before React renders, so the first frame is not the wrong colour. */
export function applyStoredTheme(): void {
  const choice = storedChoice();
  const resolved = choice === "system" ? systemTheme() : choice;
  document.documentElement.dataset.theme = resolved;
  document.documentElement.dataset.themeChoice = choice;
}

/**
 * Light, dark, or follow the system.
 *
 * The CSS defines both palettes; this only decides which one the root carries,
 * so nothing re-renders to change colour.
 */
export function useTheme() {
  const [choice, setChoice] = useState<ThemeChoice>(() => storedChoice());
  const [resolved, setResolved] = useState<ResolvedTheme>(() =>
    storedChoice() === "system" ? systemTheme() : (storedChoice() as ResolvedTheme),
  );

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const sync = () => {
      const next = choice === "system" ? systemTheme() : choice;
      setResolved(next);
      document.documentElement.dataset.theme = next;
    };
    sync();
    if (choice !== "system") {
      return;
    }
    media.addEventListener("change", sync);
    return () => media.removeEventListener("change", sync);
  }, [choice]);

  const choose = useCallback((next: ThemeChoice) => {
    window.localStorage.setItem(STORAGE_KEY, next);
    document.documentElement.dataset.themeChoice = next;
    setChoice(next);
    setResolved(next === "system" ? systemTheme() : next);
    document.documentElement.dataset.theme = next === "system" ? systemTheme() : next;
  }, []);

  /** system → light → dark → system */
  const cycle = useCallback(() => {
    choose(choice === "system" ? "light" : choice === "light" ? "dark" : "system");
  }, [choice, choose]);

  return { choice, resolved, choose, cycle };
}
