/** Everything this service says to a buyer, in one place. */
import type { Licence } from "./keys";

const PRODUCT_NAME = "Mbiro"; // placeholder, as in packages/protocol/src/constants.ts

export const strings = {
  licenceEmail: {
    subject: `Your ${PRODUCT_NAME} licence`,
    body: (key: string, licence: Licence, devices: number) =>
      [
        `Thank you for buying ${PRODUCT_NAME}.`,
        "",
        "Your licence key:",
        "",
        key,
        "",
        `To use it, open ${PRODUCT_NAME}, choose ENTER LICENCE KEY at the bottom of the window, and paste the whole key.`,
        "",
        `The licence is for ${licence.email} and works on up to ${devices} Macs. To move it to another Mac, choose REMOVE LICENCE in Settings on the old one first.`,
        "",
        "Keep this email: it is your proof of purchase.",
      ].join("\n"),
  },
};
