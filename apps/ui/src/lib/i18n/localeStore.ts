import { create } from "zustand";

import { DEFAULT_LOCALE, translate, type Locale } from "./strings";

/** The key the chosen language is kept under, which is what the next run reads. */
const KEY = "mpcraft.locale";

/** Reads the stored language, and falls back when what was stored is not a language. */
function readLocale(): Locale {
  if (typeof window === "undefined") {
    return DEFAULT_LOCALE;
  }

  try {
    const raw = window.localStorage.getItem(KEY);
    const known = ["en", "es"];

    if (raw !== null && known.includes(raw)) {
      return raw as Locale;
    }
  } catch {
    // A storage that refuses is not a reason to break the interface.
  }

  return DEFAULT_LOCALE;
}

/** Stores the language, ignoring a storage that refuses. */
function writeLocale(locale: Locale): void {
  if (typeof window === "undefined") {
    return;
  }

  try {
    window.localStorage.setItem(KEY, locale);
  } catch {
    // A storage that refuses is not a reason to break the interface.
  }
}

/**
 * The language the interface is read in.
 *
 * The language is kept in a store rather than in the menu that chooses it, because the strings
 * are read from every part of the interface at once: a sheet, a screen and the bar of the menus
 * all say the same words, and a change has to reach all three at the same time.
 */
export interface LocaleStore {
  /** The language the operator chose. */
  readonly locale: Locale;
  /** Chooses a language. */
  readonly setLocale: (locale: Locale) => void;
  /** What a string means in the language that is read. */
  readonly t: (source: string) => string;
}

export const useLocaleStore = create<LocaleStore>((set, get) => ({
  locale: readLocale(),

  setLocale: (locale) => {
    writeLocale(locale);
    set({ locale });
  },

  t: (source) => translate(get().locale, source),
}));
