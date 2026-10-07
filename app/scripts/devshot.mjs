// Pilote la fenêtre Kaleido en développement via le protocole de débogage de WebView2.
// Prérequis : lancer l'app avec WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222
//
// Usage : node scripts/devshot.mjs "<code JS à exécuter>" [capture.png] [attente ms]

// `@fichier.js` : lit le code à exécuter depuis un fichier (évite les soucis de guillemets du shell).
const [rawCode = "", out, wait = "1500"] = process.argv.slice(2);
const code = rawCode.startsWith("@") ? (await import("node:fs")).readFileSync(rawCode.slice(1), "utf8") : rawCode;

const targets = await (await fetch("http://127.0.0.1:9222/json")).json();
// KALEIDO_TARGET=companion : vise la fenêtre du compagnon.
const want = process.env.KALEIDO_TARGET;
const page = targets.find((t) => t.type === "page" && (want ? t.url.includes(want) : !t.url.includes("companion")));
if (!page) throw new Error("aucune page WebView2 trouvée");

const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  ws.onopen = resolve;
  ws.onerror = reject;
});

let nextId = 0;
const pending = new Map();
ws.onmessage = (msg) => {
  const data = JSON.parse(msg.data);
  if (pending.has(data.id)) {
    pending.get(data.id)(data);
    pending.delete(data.id);
  }
};
const send = (method, params = {}) =>
  new Promise((resolve) => {
    const id = ++nextId;
    pending.set(id, resolve);
    ws.send(JSON.stringify({ id, method, params }));
  });

if (code) {
  const res = await send("Runtime.evaluate", { expression: code, awaitPromise: true, returnByValue: true });
  const value = res.result?.result?.value ?? res.result?.exceptionDetails?.exception?.description;
  if (value !== undefined) console.log(typeof value === "string" ? value : JSON.stringify(value, null, 2));
}

if (out) {
  await new Promise((r) => setTimeout(r, Number(wait)));
  const shot = await send("Page.captureScreenshot", { format: "png" });
  const { writeFileSync } = await import("node:fs");
  writeFileSync(out, Buffer.from(shot.result.data, "base64"));
  console.log(`capture : ${out}`);
}
ws.close();
