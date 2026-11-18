import { PRODUCT_NAME, type RefusalCode } from "@mbiro/protocol";

/** Every user-facing string of the extension, in one place for translation later. */
export const strings = {
  menuDownload: `Download with ${PRODUCT_NAME}`,
  badgeOk: "OK",
  badgeFailed: "!",
  badgeOff: "OFF",
  catchingOn: `${PRODUCT_NAME} is catching downloads. Click to switch off.`,
  catchingOff: `${PRODUCT_NAME} is not catching downloads. Click to switch on.`,
  sent: `Sent to ${PRODUCT_NAME}.`,
  noAnswer: `${PRODUCT_NAME} did not answer. Open the app and try again.`,
  refused: {
    app_not_running: `${PRODUCT_NAME} is not running. Open the app and try again.`,
    invalid_message: `${PRODUCT_NAME} did not understand the request. Update the app and the extension.`,
    unsupported_version: `${PRODUCT_NAME} did not understand the request. Update the app and the extension.`,
    unsupported_url: "Only http and https links can be downloaded.",
    engine_unavailable: `${PRODUCT_NAME} cannot download right now. Restart the app.`,
    rejected: `${PRODUCT_NAME} could not start this download. Check the link.`,
  } satisfies Record<RefusalCode, string>,
} as const;
