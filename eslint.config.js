import js from "@eslint/js";
import prettier from "eslint-config-prettier";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "**/dist/",
      "**/target/",
      "**/.output/",
      "**/.wxt/",
      ".cache/",
      "design/",
      "apps/desktop/src-tauri/",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.strict,
  {
    files: ["apps/desktop/src/**/*.{ts,tsx}", "apps/extension/**/*.{ts,tsx}"],
    languageOptions: { globals: globals.browser },
    plugins: { "react-hooks": reactHooks },
    rules: reactHooks.configs.recommended.rules,
  },
  {
    files: ["**/*.config.{js,ts}", "scripts/**/*.mjs"],
    languageOptions: { globals: globals.node },
  },
  prettier,
);
