/** The one browser call the notice makes, present in every tab an extension script runs in. */
declare const chrome: { runtime: { sendMessage(message: unknown): Promise<unknown> } };

/** What the in-page notice shows and says when a button is pressed. */
export interface ToastArgs {
  /** The app's id for the download, sent back with either button. */
  id: string;
  /** The address, so the browser can download it after all. */
  url: string;
  heading: string;
  detail: string;
  name: string;
  useBrowser: string;
  show: string;
}

/** Messages the notice sends to the background. */
export type ToastMessage =
  { kind: "toast-use-browser"; id: string; url: string } | { kind: "toast-show" };

/**
 * Draws the "sent" notice in the page, as designed: bottom right, six seconds,
 * two buttons. It runs inside the page's tab, injected on demand, so it must
 * stand entirely on its own: no imports, nothing from outside its arguments.
 * It sits in a closed shadow root, so the page's styles cannot reach it and it
 * touches nothing of the page's. The page's fonts are not ours to load, so the
 * condensed heading falls back to a system face here.
 */
export function showToast(args: ToastArgs): void {
  const HOST_ID = "mbiro-notice";
  document.getElementById(HOST_ID)?.remove();

  const host = document.createElement("div");
  host.id = HOST_ID;
  host.style.cssText =
    "all:initial;position:fixed;right:16px;bottom:16px;z-index:2147483647;width:420px;max-width:calc(100vw - 32px)";
  const root = host.attachShadow({ mode: "closed" });
  const still = matchMedia("(prefers-reduced-motion: reduce)").matches;

  const style = document.createElement("style");
  style.textContent = `
    .card{box-sizing:border-box;display:flex;flex-direction:column;gap:12px;padding:16px;min-height:176px;
      background:#0C0D0E;color:#F2F4F5;border:1px solid #2A2D30;
      font-family:'IBM Plex Mono',ui-monospace,Menlo,monospace;font-variant-numeric:tabular-nums}
    .top{display:flex;justify-content:space-between;align-items:baseline;gap:12px}
    .heading{font-family:'Barlow Condensed','Arial Narrow',sans-serif;font-size:26px;font-weight:700;
      letter-spacing:.08em;color:#C6F432}
    .detail{font-size:11.5px;letter-spacing:.08em;color:#A7ADB2}
    .name{font-size:13px;font-weight:500;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
    .track{height:3px;background:#24272A}
    .bar{height:3px;background:#C6F432;width:100%}
    .buttons{margin-top:auto;display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:10px}
    button{min-height:44px;border:1px solid #3C4044;border-radius:2px;background:transparent;color:#F2F4F5;
      font:500 12px 'IBM Plex Mono',ui-monospace,Menlo,monospace;letter-spacing:.08em;cursor:pointer}
    button:hover{border-color:#A7ADB2}
    button:focus-visible{outline:2px solid #C6F432;outline-offset:2px}
    @keyframes drain{from{width:100%}to{width:0}}
    @keyframes drop{from{opacity:0;transform:translateY(-14px)}to{opacity:1;transform:none}}
    .moving .bar{animation:drain 6s linear forwards}
    .moving .top{animation:drop .7s cubic-bezier(.2,.7,.2,1) both}
  `;

  const element = (tag: string, className: string, text?: string) => {
    const node = document.createElement(tag);
    node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  };
  const card = element("div", still ? "card" : "card moving");
  card.setAttribute("role", "status");
  const top = element("div", "top");
  top.append(element("span", "heading", args.heading), element("span", "detail", args.detail));
  const track = element("div", "track");
  track.setAttribute("aria-hidden", "true");
  track.append(element("div", "bar"));
  const buttons = element("div", "buttons");
  const useBrowser = element("button", "", args.useBrowser);
  const show = element("button", "", args.show);
  buttons.append(useBrowser, show);
  card.append(top, element("span", "name", args.name), track, buttons);
  root.append(style, card);

  const close = () => host.remove();
  const timer = setTimeout(close, 6000);
  const send = (message: unknown) => {
    clearTimeout(timer);
    close();
    void chrome.runtime.sendMessage(message);
  };
  useBrowser.addEventListener("click", () =>
    send({ kind: "toast-use-browser", id: args.id, url: args.url }),
  );
  show.addEventListener("click", () => send({ kind: "toast-show" }));

  document.documentElement.append(host);
}
