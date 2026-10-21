import { useEffect, useState } from "react";

/**
 * The size of the window.
 *
 * The layout is kept as proportions of the window, and the proportions are turned into
 * pixels with the size of the window. The window is measured as it changes, so that the
 * arrangement follows the screen it is on: a column that is a fifth of the window is a
 * fifth of it on every screen.
 */
export interface WindowSize {
  /** The width of the window, in pixels. */
  readonly width: number;
  /** The height of the window, in pixels. */
  readonly height: number;
}

/** The size the window is assumed to have before it is measured. */
const ASSUMED: WindowSize = { width: 1440, height: 900 };

export function useWindowSize(): WindowSize {
  const [size, setSize] = useState<WindowSize>(ASSUMED);

  useEffect(() => {
    const measure = () => {
      setSize({ width: window.innerWidth, height: window.innerHeight });
    };

    measure();

    window.addEventListener("resize", measure);

    return () => {
      window.removeEventListener("resize", measure);
    };
  }, []);

  return size;
}
