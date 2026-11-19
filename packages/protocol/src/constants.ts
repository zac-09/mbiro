/**
 * The one TypeScript home for names that may still change.
 * All of them are placeholders until the owner confirms them (PLAN.md section 14).
 * The Rust twin is crates/common/src/constants.rs.
 */
export const PRODUCT_NAME = "Mbiro";
export const BUNDLE_IDENTIFIER = "dev.mbiro.app";
export const NATIVE_HOST_NAME = "dev.mbiro.host";
export const URL_SCHEME = "mbiro";

/** Id the Firefox extension declares for itself. */
export const FIREFOX_EXTENSION_ID = "extension@mbiro.dev";

/**
 * Public key that fixes the id of the development build of the Chrome extension
 * (jcggnjbklbbladijcfhnbhnihdpahjga). It is not a secret, and its private half
 * was never kept. The store assigns its own key and id on upload.
 */
export const CHROME_EXTENSION_KEY_DEV =
  "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAtdyM8DF6hIXoQurV/mrCBt/RZRmh7Vlzd/myd3YCEKtRGk49Uks1uNbX3fhyOfy5qW0MFEfq2/BGtuo5bcKGWrIVflnPypy3dOOMBqQ68WnbC+Ov+kWRxPClyZQa1vNr8kzjPx+yoD1ruF0AgZZYVI+3rp/Ue4WowRsLiN/exS2DtnZ+kZ4cB8ZHZWLua8ZJH0LNa07dGMkyK5rceeY8CB6tdhqrmH6bNJDwRB6JURuqC1+kGtUPhW+nTWjzfLosFCckgJs+pK912nhxLPzELfsAhSIM5HTHDs3PQFesJlveYJOH/Vrk5aZR3UlP+h75RZBd8RTex8A+PcVU9KZciwIDAQAB";

/**
 * Where the extension's "Get the app" button leads. Unset until the website's
 * address is decided (PLAN.md section 14); the button stays hidden until then.
 */
export const DOWNLOAD_PAGE_URL: string | null = null;
