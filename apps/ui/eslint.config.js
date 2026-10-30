/**
 * The lint gate for the interface.
 *
 * Formatting is prettier's job, so nothing here is a formatting rule. What is here is what a reader
 * would otherwise have to notice by hand: a name that is written and never used, a value that is
 * written and never read, a comparison that cannot be true, a dependency list that does not match
 * what the effect reads.
 *
 * What this gate does not read is named rather than left as a blind spot: the vendored MapLibre
 * worker below, and the React JSX rules, because the plugin for those cannot run on the version of
 * eslint this project uses. A rule that is not carried cannot be switched off, so no source file may
 * disable one — which is what scripts/check-lint-controls.js measures.
 */
import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "dist/",
      "playwright-report/",
      "*.log",
      // A vendored artifact, not this project's code: the minified MapLibre web worker the map runs
      // from public/. Its single line gives hundreds of findings no reader could act on, so the gate
      // reads the sources it can change and names what it leaves out.
      "public/maplibre-gl-csp-worker.js",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    plugins: { "react-hooks": reactHooks },
    rules: { "react-hooks/exhaustive-deps": "error" },
  },
  {
    // The scripts under scripts/ are run by node and, where they drive a window, evaluate code in
    // the page: both sets of globals are theirs, and neither is a name the interface declares.
    files: ["scripts/**/*.mjs"],
    languageOptions: {
      globals: {
        console: "readonly",
        document: "readonly",
        process: "readonly",
        window: "readonly",
      },
    },
  },
);
