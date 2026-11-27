/** Everything this service says to a buyer, in one place. */
import type { Licence } from "./keys";

// Placeholders, as in packages/protocol/src/constants.ts.
const PRODUCT_NAME = "Mbiro";
export const URL_SCHEME = "mbiro";

export const strings = {
  licenceEmail: {
    subject: `Your ${PRODUCT_NAME} licence`,
    /** `link` is the one-click activation link, when the service knows its own address. */
    body: (key: string, licence: Licence, devices: number, link: string | null) =>
      [
        `Thank you for buying ${PRODUCT_NAME}.`,
        "",
        ...(link
          ? [
              `To activate it, open this link on the Mac where ${PRODUCT_NAME} is installed:`,
              "",
              link,
              "",
              "Or enter the key yourself:",
            ]
          : ["Your licence key:"]),
        "",
        key,
        "",
        `${link ? "To enter it" : "To use it"}, open ${PRODUCT_NAME}, choose ENTER LICENCE KEY at the bottom of the window, and paste the whole key.`,
        "",
        `The licence is for ${licence.email} and works on up to ${devices} Macs. To move it to another Mac, choose REMOVE LICENCE in Settings on the old one first.`,
        "",
        "Keep this email: it is your proof of purchase.",
      ].join("\n"),
  },
  activationPage: {
    title: `Activate ${PRODUCT_NAME}`,
    heading: `OPENING ${PRODUCT_NAME.toUpperCase()}`,
    body: `If your browser asks, allow it to open ${PRODUCT_NAME}. The app activates your licence by itself.`,
    open: `OPEN ${PRODUCT_NAME.toUpperCase()}`,
    notInstalled: `Nothing happened? ${PRODUCT_NAME} must be installed on this Mac first. You can also paste the key from your licence email into the app.`,
    noKey: `This link has no licence key in it. Use the link in your licence email, or paste the key from that email into ${PRODUCT_NAME}.`,
  },
};
