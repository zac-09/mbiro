import { CHROME_EXTENSION_KEY_DEV, FIREFOX_EXTENSION_ID, PRODUCT_NAME } from "@mbiro/protocol";
import react from "@vitejs/plugin-react";
import { defineConfig } from "wxt";

// The extension only hands ordinary file downloads to the app. It has no media
// handling of any kind and no script that runs in pages on its own. Host access
// is needed for two things: reading the cookies the browser would send with a
// download, and drawing the "sent" notice in the tab after one is caught.
export default defineConfig({
  // A visible folder: the browser's file picker hides dot-folders such as .output.
  outDir: "dist",
  manifestVersion: 3,
  vite: () => ({ plugins: [react()] }),
  manifest: ({ browser }) => ({
    name: PRODUCT_NAME,
    description: `Send file downloads to the ${PRODUCT_NAME} app.`,
    permissions: [
      "nativeMessaging",
      "contextMenus",
      "downloads",
      "cookies",
      "storage",
      "scripting",
    ],
    host_permissions: ["<all_urls>"],
    action: { default_title: PRODUCT_NAME },
    ...(browser === "firefox"
      ? { browser_specific_settings: { gecko: { id: FIREFOX_EXTENSION_ID } } }
      : { key: CHROME_EXTENSION_KEY_DEV }),
  }),
});
