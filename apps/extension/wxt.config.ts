import { CHROME_EXTENSION_KEY_DEV, FIREFOX_EXTENSION_ID, PRODUCT_NAME } from "@mbiro/protocol";
import { defineConfig } from "wxt";

// The extension only hands ordinary file links to the app. It has no host
// permissions, no content scripts and no media handling of any kind.
export default defineConfig({
  manifestVersion: 3,
  manifest: ({ browser }) => ({
    name: PRODUCT_NAME,
    description: `Send file downloads to the ${PRODUCT_NAME} app.`,
    permissions: ["nativeMessaging", "contextMenus"],
    action: { default_title: PRODUCT_NAME },
    ...(browser === "firefox"
      ? { browser_specific_settings: { gecko: { id: FIREFOX_EXTENSION_ID } } }
      : { key: CHROME_EXTENSION_KEY_DEV }),
  }),
});
