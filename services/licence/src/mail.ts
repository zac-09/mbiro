/** Sending the licence email. The mail provider is one adapter, not chosen yet. */

export interface Message {
  to: string;
  subject: string;
  text: string;
}

export interface Mailer {
  /** Rejects if the message was not accepted for delivery. */
  send(message: Message): Promise<void>;
}
