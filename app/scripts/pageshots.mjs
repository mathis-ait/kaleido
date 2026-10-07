// Captures de toutes les pages de l'éditeur dans plusieurs thèmes et tailles de fenêtre.
// Prérequis : app lancée avec WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222
//
// Usage : node scripts/pageshots.mjs <dossier de sortie> <sauvegarde> [préfixe] [pages] [thèmes] [largeurs]
//   pages    : liste séparée par des virgules (défaut : toutes)
//   thèmes   : défaut lagon,reseau,pixel,nuit
//   largeurs : défaut 1280,1920
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [outDir, savePath, prefix = "", pagesArg, themesArg, widthsArg] = process.argv.slice(2);
if (!outDir || !savePath) throw new Error("usage : pageshots.mjs <sortie> <sauvegarde> [préfixe] [pages] [thèmes] [largeurs]");

// Chaque page : comment l'afficher (code évalué dans la page).
const PAGES = {
  home: `__kaleido.saveStore.goTo("home")`,
  boxes: `__kaleido.saveStore.goTo("boxes")`,
  pokemon: `__kaleido.saveStore.goTo("pokemon")`,
  bank: `__kaleido.saveStore.goTo("bank")`,
  nuzlocke: `__kaleido.saveStore.goTo("nuzlocke")`,
  battle: `__kaleido.saveStore.goTo("battle")`,
  gifts: `__kaleido.saveStore.goTo("gifts")`,
  encounters: `__kaleido.saveStore.goTo("encounters")`,
  showdown: `(async () => { __kaleido.saveStore.goTo("boxes"); const m = await import("/src/save/showdown/api.ts"); m.openShowdown?.("export"); })()`,
  teams: `(async () => { __kaleido.saveStore.goTo("boxes"); const m = await import("/src/save/showdown/api.ts"); m.showdownUi.teams = true; })()`,
  smogon: `(async () => { __kaleido.saveStore.goTo("boxes"); const m = await import("/src/save/showdown/api.ts"); m.showdownUi.smogon = true; })()`,
};
const pages = pagesArg ? pagesArg.split(",") : Object.keys(PAGES);
const themes = (themesArg ?? "lagon,reseau,pixel,nuit").split(",");
const widths = (widthsArg ?? "1280,1920").split(",").map(Number);

const targets = await (await fetch("http://127.0.0.1:9222/json")).json();
const page = targets.find((t) => t.type === "page" && !t.url.includes("companion"));
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
const evaluate = async (expression) => {
  const res = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (res.result?.exceptionDetails) console.error(res.result.exceptionDetails.exception?.description);
  return res.result?.result?.value;
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

mkdirSync(outDir, { recursive: true });
await evaluate(`(async () => { __kaleido.nav.view = "saves"; await __kaleido.openSave(${JSON.stringify(savePath)}); })()`);
await sleep(1500);

// Ferme les dialogues éventuellement ouverts entre deux captures.
const reset = `(async () => { const m = await import("/src/save/showdown/api.ts"); for (const k of Object.keys(m.showdownUi)) if (typeof m.showdownUi[k] === "boolean") m.showdownUi[k] = false; })()`;

for (const width of widths) {
  const height = width === 1920 ? 1080 : 820;
  await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
  for (const theme of themes) {
    await evaluate(`__kaleido.currentTheme.value = ${JSON.stringify(theme)}`);
    for (const name of pages) {
      await evaluate(reset);
      await evaluate(PAGES[name]);
      await sleep(1200);
      const shot = await send("Page.captureScreenshot", { format: "png" });
      const file = join(outDir, `${prefix}${name}-${theme}-${width}.png`);
      writeFileSync(file, Buffer.from(shot.result.data, "base64"));
      console.log(file);
    }
  }
}
await evaluate(reset);
await send("Emulation.clearDeviceMetricsOverride");
ws.close();
