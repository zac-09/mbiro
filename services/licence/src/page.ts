/**
 * The page behind the activation link in the licence email. Mail programs do
 * not reliably make a link with the app's own scheme clickable, so the email
 * carries an ordinary https link to this page, and the page hands over to the
 * app.
 *
 * The key rides in the part of the address after "#", which browsers never
 * send to a server: this service does not see the key when the page is opened,
 * and it cannot end up in an access log. The page takes the key out of the
 * address bar as soon as it has read it.
 */
import { createHash } from "node:crypto";
import { URL_SCHEME, strings } from "./strings";

const t = strings.activationPage;

const ENTITIES: Record<string, string> = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
};
const escape = (text: string) => text.replace(/[&<>"]/g, (c) => ENTITIES[c] ?? c);

/** Only something shaped like a key is passed to the app. */
const script = `
const key = location.hash.slice(1);
const ok = /^[A-Za-z0-9_-]{1,1024}\\.[A-Za-z0-9_-]{1,1024}$/.test(key);
document.getElementById(ok ? "found" : "missing").hidden = false;
if (ok) {
  const link = ${JSON.stringify(`${URL_SCHEME}://activate?key=`)} + key;
  document.getElementById("open").href = link;
  history.replaceState(null, "", location.pathname);
  location.href = link;
}
`;

const style = `
body { margin: 0; min-height: 100vh; display: grid; place-items: center; background: #0C0D0E;
  color: #E8E6E1; font: 15px/1.55 ui-monospace, Menlo, monospace; }
main { box-sizing: border-box; width: 100%; max-width: 520px; padding: 32px 16px; }
h1 { margin: 0 0 20px; font-size: 28px; letter-spacing: 0.06em; }
a { display: inline-block; margin: 8px 0 20px; padding: 14px 28px; border-radius: 2px;
  background: #E8E6E1; color: #0C0D0E; font-weight: 700; letter-spacing: 0.08em; text-decoration: none; }
.quiet { color: #9A978F; font-size: 13px; }
`;

const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>${escape(t.title)}</title>
<style>${style}</style>
</head>
<body>
<main>
<div id="found" hidden>
<h1>${escape(t.heading)}</h1>
<p>${escape(t.body)}</p>
<a id="open" href="#">${escape(t.open)}</a>
<p class="quiet">${escape(t.notInstalled)}</p>
</div>
<p id="missing" hidden>${escape(t.noKey)}</p>
<noscript><p>${escape(t.noKey)}</p></noscript>
</main>
<script>${script}</script>
</body>
</html>
`;

const hash = (text: string) => `'sha256-${createHash("sha256").update(text).digest("base64")}'`;

const headers = {
  "content-type": "text/html; charset=utf-8",
  // The page is the same for everyone; the key never reaches the server.
  "cache-control": "public, max-age=3600",
  "content-security-policy": `default-src 'none'; script-src ${hash(script)}; style-src ${hash(style)}; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`,
  "referrer-policy": "no-referrer",
  "x-content-type-options": "nosniff",
};

export const activationPage = () => new Response(html, { status: 200, headers });

/** The link the licence email carries, given where the service is reached. */
export const activationLink = (publicUrl: string, key: string) =>
  `${publicUrl.replace(/\/+$/, "")}/activate#${key}`;
