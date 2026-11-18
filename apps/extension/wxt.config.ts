import { CHROME_EXTENSION_KEY_DEV, FIREFOX_EXTENSION_ID, PRODUCT_NAME } from "@mbiro/protocol";
import { defineConfig } from "wxt";

// The extension only hands ordinary file downloads to the app. It has no content
// scripts and no media handling of any kind. Host access is needed for one
// thing: reading the cookies the browser would send with a download.
export default defineConfig({
  // A visible folder: the browser's file picker hides dot-folders such as .output.
  outDir: "dist",
  manifestVersion: 3,
  manifest: ({ browser }) => ({
    name: PRODUCT_NAME,
    description: `Send file downloads to the ${PRODUCT_NAME} app.`,
    permissions: ["nativeMessaging", "contextMenus", "downloads", "cookies", "storage"],
    host_permissions: ["<all_urls>"],
    action: { default_title: PRODUCT_NAME },
    ...(browser === "firefox"
      ? { browser_specific_settings: { gecko: { id: FIREFOX_EXTENSION_ID } } }
      : { key: CHROME_EXTENSION_KEY_DEV }),
  }),
});
