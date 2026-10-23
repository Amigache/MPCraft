import { beforeEach, describe, expect, it, vi } from "vitest";

import { SCREENS, TABS } from "../ui/uiStore";
import { DEFAULT_LOCALE, entry, LOCALES, tagFor, translate } from "./strings";

/**
 * The store of the language, with a storage that holds what the last run left behind.
 *
 * A stored language can be something that is not a language, and the store has to fall back
 * rather than read strings from a bundle that does not exist. The store takes its first value
 * when the module is imported, so it is built again for each case.
 */
async function storeWithStoredLocale(stored: string | null) {
  const values = new Map<string, string>();

  if (stored !== null) {
    values.set("mpcraft.locale", stored);
  }

  (globalThis as { window?: unknown }).window = {
    localStorage: {
      getItem: (key: string) =>
        values.has(key) ? (values.get(key) as string) : null,
      setItem: (key: string, value: string) => {
        values.set(key, value);
      },
    },
  };

  vi.resetModules();
  const module = await import("./localeStore");

  return { store: module.useLocaleStore, values };
}

beforeEach(() => {
  vi.resetModules();
});

describe("the strings the interface says", () => {
  it("are said in the language that is read", () => {
    expect(translate("es", "Flight data")).toBe("Datos del vuelo");
    expect(translate("es", "Write a diagnostics bundle…")).toBe(
      "Escribir un paquete de diagnóstico…",
    );
  });

  it("are said as they were written when the language has nothing for them", () => {
    // A part of the interface that has not been translated yet still says something, and it says
    // what it said before, which is better than a name that is blank.
    expect(translate("es", "The graph of the altitude")).toBe(
      "The graph of the altitude",
    );
  });

  it("are said the way the code wrote them in English", () => {
    expect(translate("en", "Flight data")).toBe("Flight data");
  });

  it("carry a translation for every name the switcher and the tabs show", () => {
    const names = [
      ...SCREENS.map((screen) => screen.label),
      ...TABS.map((tab) => tab.label),
      // The names the bar of the state shows along the bottom.
      "Mode",
      "Armed",
      "To home",
      "Received",
      "Lost",
    ];

    for (const name of names) {
      expect(entry("es", name), `the name ${name} has no entry`).toBeDefined();
    }
  });

  it("says each language in its own words, so that both can be told apart", () => {
    expect(LOCALES.map((locale) => locale.label)).toEqual([
      "English",
      "Español",
    ]);
  });

  it("puts the values where the sentence has a gap", () => {
    expect(
      translate("es", "The folder holds {count} pictures", { count: 6 }),
    ).toBe("The folder holds 6 pictures");

    // A gap with no value is left as it was written, which is better than a missing word nobody
    // can see.
    expect(translate("es", "The folder holds {count} pictures", {})).toBe(
      "The folder holds {count} pictures",
    );
  });

  it("writes a number the way the language writes it", () => {
    expect(tagFor("en")).toBe("en-US");
    expect(tagFor("es")).toBe("es-ES");
  });
});

describe("the language the interface is read in", () => {
  it("falls back when nothing was stored", async () => {
    const { store } = await storeWithStoredLocale(null);

    expect(store.getState().locale).toBe(DEFAULT_LOCALE);
  });

  it("reads what the last run left behind", async () => {
    const { store } = await storeWithStoredLocale("es");

    expect(store.getState().locale).toBe("es");
    expect(store.getState().tag).toBe("es-ES");
    expect(store.getState().t("Flight data")).toBe("Datos del vuelo");
  });

  it("does not read something that is not a language", async () => {
    const { store } = await storeWithStoredLocale("klingon");

    expect(store.getState().locale).toBe(DEFAULT_LOCALE);
  });

  it("keeps what was stored when the language is chosen", async () => {
    const { store, values } = await storeWithStoredLocale(null);

    store.getState().setLocale("es");

    expect(store.getState().locale).toBe("es");
    expect(values.get("mpcraft.locale")).toBe("es");
  });
});
