// Génère les données de la Living Dex (`app/public/livedex/`) à partir des données de
// Pelagix (HydrosPlays, GPLv3 — elles-mêmes tirées de PKHeX et de PokeAPI), traduites en
// français avec les textes de PKHeX (kwsch, GPLv3).
//
// Usage :
//   node tools/livedex/build-data.mjs <pelagix>/src/renderer/public/data <PKHeX>/PKHeX.Core/Resources/text
//
// Sortie : `dex.json` (index : jeux, espèces, formes) et `species/<n°>.json` (où le trouver).

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const [, , pelagixDir, pkhexText] = process.argv;
if (!pelagixDir || !pkhexText) {
  console.error("usage : node tools/livedex/build-data.mjs <données Pelagix> <textes PKHeX>");
  process.exit(1);
}
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const outDir = path.join(root, "app/public/livedex");

const readJson = (p) => JSON.parse(fs.readFileSync(p, "utf8"));
const lines = (p) => fs.readFileSync(p, "utf8").replace(/^﻿/, "").split(/\r?\n/);
/** Apostrophes typographiques et espaces insécables ramenées à une forme simple pour comparer. */
const norm = (s) => s.replace(/[’‘]/g, "'").replace(/ /g, " ").trim();

/** Table anglais → français à partir de deux fichiers PKHeX alignés ligne à ligne. */
function pairMap(enFile, frFile, into = new Map()) {
  const en = lines(enFile);
  const fr = lines(frFile);
  en.forEach((e, i) => {
    const k = norm(e);
    if (k && fr[i] && !into.has(k)) into.set(k, fr[i].trim());
  });
  return into;
}

const other = (name, lang) => path.join(pkhexText, "other", lang, `text_${name}_${lang}.txt`);
const speciesFr = lines(other("Species", "fr"));
const speciesEn = lines(other("Species", "en"));
const items = pairMap(path.join(pkhexText, "items", "text_Items_en.txt"), path.join(pkhexText, "items", "text_Items_fr.txt"));
const moves = pairMap(other("Moves", "en"), other("Moves", "fr"));
const formsMap = pairMap(other("Forms", "en"), other("Forms", "fr"));
const typesMap = pairMap(other("Types", "en"), other("Types", "fr"));

// Lieux : tous les fichiers `locations/gen*/text_<jeu>_<banque>_en.txt` qui ont un pendant français.
const places = new Map();
const locDir = path.join(pkhexText, "locations");
for (const gen of fs.readdirSync(locDir)) {
  for (const f of fs.readdirSync(path.join(locDir, gen))) {
    if (!f.endsWith("_en.txt")) continue;
    const fr = path.join(locDir, gen, f.replace(/_en\.txt$/, "_fr.txt"));
    if (fs.existsSync(fr)) pairMap(path.join(locDir, gen, f), fr, places);
  }
}

// --- Jeux (ordre et identifiants de Pelagix ; noms français, `version` = GameVersion de PKHeX).
const GAMES = {
  red: ["Pokémon Rouge", "Rouge", "gb", 1, 35],
  green: ["Pokémon Vert", "Vert", "gb", 1, 36],
  blue: ["Pokémon Bleu", "Bleu", "gb", 1, 37],
  yellow: ["Pokémon Jaune", "Jaune", "gb", 1, 38],
  stadium: ["Pokémon Stadium", "Stadium", "n64", 1, null],
  gold: ["Pokémon Or", "Or", "gbc", 2, 39],
  silver: ["Pokémon Argent", "Argent", "gbc", 2, 40],
  crystal: ["Pokémon Cristal", "Cristal", "gbc", 2, 41],
  stadium2: ["Pokémon Stadium 2", "Stadium 2", "n64", 2, null],
  ruby: ["Pokémon Rubis", "Rubis", "gba", 3, 2],
  sapphire: ["Pokémon Saphir", "Saphir", "gba", 3, 1],
  boxrubysapphire: ["Pokémon Box Rubis & Saphir", "Box", "gcn", 3, null],
  colosseum: ["Pokémon Colosseum", "Colosseum", "gcn", 3, 15],
  firered: ["Pokémon Rouge Feu", "Rouge Feu", "gba", 3, 4],
  leafgreen: ["Pokémon Vert Feuille", "Vert Feuille", "gba", 3, 5],
  emerald: ["Pokémon Émeraude", "Émeraude", "gba", 3, 3],
  xd: ["Pokémon XD : Le Souffle des Ténèbres", "XD", "gcn", 3, 15],
  diamond: ["Pokémon Diamant", "Diamant", "nds", 4, 10],
  pearl: ["Pokémon Perle", "Perle", "nds", 4, 11],
  platinum: ["Pokémon Platine", "Platine", "nds", 4, 12],
  heartgold: ["Pokémon Or HeartGold", "HeartGold", "nds", 4, 7],
  soulsilver: ["Pokémon Argent SoulSilver", "SoulSilver", "nds", 4, 8],
  black: ["Pokémon Noire", "Noire", "nds", 5, 21],
  white: ["Pokémon Blanche", "Blanche", "nds", 5, 20],
  black2: ["Pokémon Noire 2", "Noire 2", "nds", 5, 23],
  white2: ["Pokémon Blanche 2", "Blanche 2", "nds", 5, 22],
  x: ["Pokémon X", "X", "3ds", 6, 24],
  y: ["Pokémon Y", "Y", "3ds", 6, 25],
  omegaruby: ["Pokémon Rubis Oméga", "Rubis Oméga", "3ds", 6, 27],
  alphasapphire: ["Pokémon Saphir Alpha", "Saphir Alpha", "3ds", 6, 26],
  sun: ["Pokémon Soleil", "Soleil", "3ds", 7, 30],
  moon: ["Pokémon Lune", "Lune", "3ds", 7, 31],
  ultrasun: ["Pokémon Ultra-Soleil", "Ultra-Soleil", "3ds", 7, 32],
  ultramoon: ["Pokémon Ultra-Lune", "Ultra-Lune", "3ds", 7, 33],
  letsgopikachu: ["Pokémon : Let's Go, Pikachu", "Let's Go Pikachu", "switch", 7, 42],
  letsgoeevee: ["Pokémon : Let's Go, Évoli", "Let's Go Évoli", "switch", 7, 43],
  sword: ["Pokémon Épée", "Épée", "switch", 8, 44],
  shield: ["Pokémon Bouclier", "Bouclier", "switch", 8, 45],
  brilliantdiamond: ["Pokémon Diamant Étincelant", "Diamant Étincelant", "switch", 8, 48],
  shiningpearl: ["Pokémon Perle Scintillante", "Perle Scintillante", "switch", 8, 49],
  legendsarceus: ["Légendes Pokémon : Arceus", "Légendes : Arceus", "switch", 8, 47],
  scarlet: ["Pokémon Écarlate", "Écarlate", "switch", 9, 50],
  violet: ["Pokémon Violet", "Violet", "switch", 9, 51],
  legendsza: ["Légendes Pokémon : Z-A", "Légendes : Z-A", "switch", 9, 52],
  go: ["Pokémon GO", "GO", "mobile", 0, 34],
  home: ["Pokémon HOME", "HOME", "mobile", 0, null],
};

// --- Méthodes et conditions de rencontre.
const METHODS = {
  "Max Raid Den": "Antre Dynamax",
  "Tall grass": "Hautes herbes",
  "Event Mass Outbreak": "Apparition massive (évènement)",
  "Mystery Gift": "Cadeau Mystère",
  Overworld: "Sur la carte",
  "Super Rod": "Super Canne",
  Cave: "Grotte",
  "Pokémon GO": "Pokémon GO",
  Surfing: "Surf",
  "SOS call": "Appel à l'aide",
  "Mass Outbreak": "Apparition massive",
  "Grand Underground": "Grands Souterrains",
  Hyperspace: "Hyperespace",
  "Fixed spawn": "Apparition fixe",
  "Massive Mass Outbreak": "Méga-apparition massive",
  "Good Rod": "Canne",
  "Shaking tree or ore deposit": "Arbre ou gisement qui tremble",
  Headbutt: "Coup d'Boule",
  "Wild Zone": "Zone sauvage",
  "Event Max Raid": "Raid Dynamax (évènement)",
  "Old Rod": "Canne à pêche",
  "Dynamax Adventure": "Aventure Dynamax",
  "Overworld (water)": "Sur la carte (eau)",
  "Dream World": "Pokémon Dream World",
  "Wild Tera Pokémon": "Pokémon Téracristal sauvage",
  "Friend Safari": "Safari des Amis",
  "In-game trade": "Échange en jeu",
  "Safari Zone": "Parc Safari",
  "Shadow Pokémon": "Pokémon Obscur",
  "Honey Tree": "Arbre à Miel",
  Gift: "Don",
  Horde: "Horde",
  Fishing: "Pêche",
  "Space-time Distortion": "Distorsion spatio-temporelle",
  "Pokémon Center New York": "Pokémon Center New York",
  "5★ Tera Raid": "Raid Téracristal 5★",
  "4★ Tera Raid": "Raid Téracristal 4★",
  "Headbutt (special tree)": "Coup d'Boule (arbre spécial)",
  "3★ Tera Raid": "Raid Téracristal 3★",
  "6★ Tera Raid": "Raid Téracristal 6★",
  "Event distribution": "Distribution",
  "Event Tera Raid": "Raid Téracristal (évènement)",
  "1★ Tera Raid": "Raid Téracristal 1★",
  "2★ Tera Raid": "Raid Téracristal 2★",
  "Hidden Grotto": "Trouée Cachée",
  Fossil: "Fossile",
  "Island Scan": "Scanner d'île",
  "Berry tree": "Baies",
  "Pokémon Center (Japan)": "Pokémon Center (Japon)",
  "Rock Smash": "Éclate-Roc",
  Starter: "Starter",
  Swarm: "Essaim",
  "7★ Tera Raid": "Raid Téracristal 7★",
  "Overworld (flying)": "Sur la carte (en vol)",
  "Poké Pelago": "Poké Loisir",
  "Game Corner prize": "Lot du Casino",
  "Great Marsh": "Grand Marais",
  "Ambush encounter": "Embuscade",
  "Global Link promotion": "Promotion du Pokémon Global Link",
  "Pokémon HOME gift": "Cadeau de Pokémon HOME",
  "Bug-Catching Contest": "Concours de Capture d'Insectes",
  "Dream Radar": "Radar Rêve",
  "Safari Zone (Surfing)": "Parc Safari (Surf)",
  "Event encounter": "Rencontre d'évènement",
  "My Pokémon Ranch": "My Pokémon Ranch",
  "Safari Zone (Super Rod)": "Parc Safari (Super Canne)",
  Roaming: "Errant",
  "Safari Zone (Good Rod)": "Parc Safari (Canne)",
  "N's Pokémon": "Pokémon de N",
  "Dynamax Crystal den": "Antre (Cristal Dynamax)",
  Totem: "Pokémon Dominant",
  "Safari Zone (Old Rod)": "Parc Safari (Canne à pêche)",
  "Pokémon Stadium": "Pokémon Stadium",
  "Poké Spot": "Poké Spot",
  "Pokémon Ranger": "Pokémon Ranger",
  "Titan Pokémon": "Pokémon Colossal",
  "Great Marsh (Super Rod)": "Grand Marais (Super Canne)",
  "Bonus Disc": "Disque bonus",
  "Great Marsh (Surfing)": "Grand Marais (Surf)",
  "Great Marsh (Good Rod)": "Grand Marais (Canne)",
  "Pokémon Box": "Pokémon Box",
  "Berry Glitch fix": "Correctif du bug des Baies",
  "Safari Zone (Rock Smash)": "Parc Safari (Éclate-Roc)",
  "Pokémon Stadium 2": "Pokémon Stadium 2",
  "Great Marsh (Old Rod)": "Grand Marais (Canne à pêche)",
  "Virtual Console": "Console virtuelle",
  "Pokémon Channel": "Pokémon Channel",
};
const CONDITIONS = {
  Egg: "Œuf",
  "Japanese games": "Jeux japonais",
  Monday: "Lundi",
  Tuesday: "Mardi",
  Wednesday: "Mercredi",
  Thursday: "Jeudi",
  Friday: "Vendredi",
  Saturday: "Samedi",
  Sunday: "Dimanche",
  Alpha: "Baron",
  Morning: "Matin",
  Day: "Jour",
  Evening: "Soir",
  Night: "Nuit",
  Overcast: "Nuageux",
  Rain: "Pluie",
  Thunderstorm: "Orage",
  Fog: "Brouillard",
  Sandstorm: "Tempête de sable",
  Clear: "Beau temps",
  "Harsh sunlight": "Soleil intense",
  Snow: "Neige",
  Snowstorm: "Tempête de neige",
  "Feebas tiles": "Cases de Barpau",
  Swarm: "Essaim",
};
const methodFr = (m) => {
  if (METHODS[m]) return METHODS[m];
  const walker = /^Pokéwalker - (.*)$/.exec(m);
  if (walker) return `Pokéwalker · ${places.get(norm(walker[1])) ?? walker[1]}`;
  return m;
};
const conditionFr = (c) => CONDITIONS[c] ?? c;

/** Lieu anglais → français ; « Lieu (Zone) » traduit en deux morceaux. */
function placeFr(s) {
  const k = norm(s);
  if (places.has(k)) return places.get(k);
  const m = /^(.*) \((.*)\)$/.exec(k);
  if (m) return `${placeFr(m[1])} (${placeFr(m[2])})`;
  const special = { Paldea: "Paldea", Kitakami: "Septentria", "Wild Area": "Terres Sauvages", "Isle of Armor": "Isolarmure", "Crown Tundra": "Terres Enneigées" };
  return special[k] ?? s;
}

// --- Évolutions : phrases courantes, objets et attaques traduits.
const itemFr = (s) => items.get(norm(s)) ?? s;
const moveFr = (s) => moves.get(norm(s)) ?? s;
function howFr(how) {
  let s = how;
  const parts = [];
  // Préfixes de changement de forme.
  let m;
  if ((m = /^Change form: (.*)$/.exec(s))) return `Changement de forme : ${howTail(m[1])}`;
  if ((m = /^Fuse with (.*)$/.exec(s))) return `Fusion avec ${speciesNameFr(m[1])}`;
  if ((m = /^Level (\d+)(.*)$/.exec(s))) {
    parts.push(`Niveau ${m[1]}`);
    s = m[2];
  } else if ((m = /^Use (?:a |an )?(.+?)( in .*| during .*| at night.*)?$/.exec(s)) && !/ times/.test(s)) {
    parts.push(`Utiliser ${itemFr(m[1])}`);
    s = m[2] ?? "";
  } else if ((m = /^Trade holding (?:a |an )?(.+)$/.exec(s))) {
    return `Échange en tenant ${itemFr(m[1])}`;
  } else if ((m = /^Trade for (.+)$/.exec(s))) {
    return `Échange contre ${speciesNameFr(m[1])}`;
  } else if (s === "Trade") {
    return "Échange";
  } else if ((m = /^(?:Evolve|Level up) with high friendship(.*)$/.exec(s))) {
    parts.push("Niveau avec beaucoup d'amitié");
    s = m[1];
  } else if ((m = /^(?:Evolve|Level up) with high affection(.*)$/.exec(s))) {
    parts.push("Niveau avec beaucoup d'affection");
    s = m[1];
  } else if ((m = /^(?:Evolve|Level up)(.*)$/.exec(s))) {
    parts.push("Monter d'un niveau");
    s = m[1];
  } else {
    return how;
  }
  if (s) parts.push(howTail(s.trim()));
  return parts.join(" ");
}
function howTail(s) {
  return s
    .replace(/^knowing a ([A-Za-z]+)-type move/, (_, t) => `en connaissant une attaque de type ${typesMap.get(t) ?? t}`)
    .replace(/^knowing (.+?)(?=$| \()/, (_, mv) => `en connaissant ${moveFr(mv)}`)
    .replace(/^holding (?:a |an )?(.+?)(?= at night| during the day|$)/, (_, it) => `en tenant ${itemFr(it)}`)
    .replace(/ ?at night/, " la nuit")
    .replace(/ ?during the day/, " le jour")
    .replace(/^in Ultra Space/, "dans l'Ultra-Dimension")
    .replace(/^with Attack higher than Defense/, "si Attaque > Défense")
    .replace(/^with Defense higher than Attack/, "si Défense > Attaque")
    .replace(/^with Attack equal to Defense/, "si Attaque = Défense")
    .trim();
}
function speciesNameFr(en) {
  const i = speciesEn.findIndex((n) => norm(n) === norm(en));
  return i > 0 ? speciesFr[i] : en;
}

// --- Formes.
const REGION = { alola: "d'Alola", galar: "de Galar", hisui: "de Hisui", paldea: "de Paldea" };
function formNameFr(name, speciesId, cat, region) {
  if (!name) return "";
  if (formsMap.has(norm(name))) return formsMap.get(norm(name));
  if (cat === "regional" && region) {
    const inner = /\((.*)\)$/.exec(name);
    return `Forme ${REGION[region]}${inner ? ` (${formsMap.get(norm(inner[1])) ?? inner[1]})` : ""}`;
  }
  let m;
  if ((m = /^Type: (.*)$/.exec(name))) return typesMap.get(m[1]) ?? name;
  // « Icy Snow Pattern », « Red Flower », « Heart Trim »… : PKHeX ne nomme que la première partie.
  if ((m = /^(.*) (Pattern|Flower|Trim|Size|Mode|Style|Drive|Meteor|Core|Plumage|Build|Mask|Color)$/.exec(name)) && formsMap.has(norm(m[1]))) {
    return formsMap.get(norm(m[1]));
  }
  if ((m = /^Mega (.+?)( [XYZ])?$/.exec(name))) return `Méga-${speciesFr[speciesId]}${m[2] ?? ""}`;
  if (name === "Primal Reversion") return "Primo-Résurgence";
  if ((m = /^(.*) Rotom$/.exec(name))) return formsMap.get(m[1]) ?? name;
  if ((m = /^(.*) Type$/.exec(name))) return typesMap.get(m[1]) ?? name;
  if ((m = /^(.*) Forme?$/.exec(name))) return formsMap.get(norm(m[1])) ?? `Forme ${m[1]}`;
  if ((m = /^(.*) Cloak$/.exec(name))) return formsMap.get(norm(name)) ?? `Cape ${m[1]}`;
  if ((m = /^(.*) Cap$/.exec(name))) return `Casquette ${m[1]}`;
  if (name === "Partner") return "Partenaire";
  if (name === "Totem Alolan" || name === "Totem") return "Dominant";
  return name;
}
function formFullFr(sp, f, nameFr) {
  const base = speciesFr[sp.id];
  if (!nameFr) return base;
  if (f.cat === "regional" && f.region) {
    const inner = /\((.*)\)$/.exec(nameFr);
    return `${base} ${REGION[f.region]}${inner ? ` (${inner[1]})` : ""}`;
  }
  if (f.cat === "mega" || nameFr === "Primo-Résurgence") return nameFr === "Primo-Résurgence" ? `Primo-${base}` : nameFr;
  return `${base} (${nameFr})`;
}

// --- Construction.
const dex = readJson(path.join(pelagixDir, "dex.json"));
const games = dex.games.map((id) => {
  const g = GAMES[id];
  if (!g) throw new Error(`jeu inconnu : ${id}`);
  return { id, name: g[0], short: g[1], system: g[2], gen: g[3], version: g[4] };
});
const untranslated = { forms: new Set(), how: new Set(), places: new Set() };

const species = dex.species.map((sp) => ({
  id: sp.id,
  name: speciesFr[sp.id],
  en: sp.name,
  gen: sp.gen,
  tags: sp.tags,
  genderRate: sp.genderRate,
  genderDiff: sp.genderDiff,
  family: sp.family,
  forms: sp.forms.map((f) => {
    const name = formNameFr(f.name, sp.id, f.cat, f.region);
    if (name === f.name && f.name && !/^[A-Z!?]$/.test(f.name)) untranslated.forms.add(f.name);
    const out = { f: f.f, name, full: formFullFr(sp, f, name), cat: f.cat, types: f.types, present: f.present, obtain: f.obtain, event: f.event };
    if (f.region) out.region = f.region;
    if (f.female) out.female = true;
    if (f.gmax) out.gmax = true;
    if (f.gender) out.gender = f.gender;
    if (f.go) out.go = f.go;
    if (f.variants) out.variants = f.variants.map((v) => ({ id: v.id, name: v.name }));
    return out;
  }),
}));

fs.rmSync(outDir, { recursive: true, force: true });
fs.mkdirSync(path.join(outDir, "species"), { recursive: true });
fs.writeFileSync(
  path.join(outDir, "dex.json"),
  JSON.stringify({ meta: { ...dex.meta, source: "Pelagix (HydrosPlays, GPLv3)", builtAt: new Date().toISOString() }, games, gameBalls: dex.gameBalls, species }),
);

let rows = 0;
for (const sp of dex.species) {
  const d = readJson(path.join(pelagixDir, "species", `${sp.id}.json`));
  const strings = d.strings.map((s) => {
    const fr = placeFr(s);
    if (fr === s) untranslated.places.add(s);
    return fr;
  });
  const tr = (h) => {
    const fr = howFr(h);
    if (fr === h) untranslated.how.add(h);
    return fr;
  };
  const forms = {};
  for (const [f, fd] of Object.entries(d.forms)) {
    rows += fd.rows.length;
    forms[f] = {
      rows: fd.rows.map((r) => {
        const o = { ...r };
        if (r.m) o.m = methodFr(r.m);
        if (r.c) o.c = r.c.map(conditionFr);
        return o;
      }),
      evolve: fd.evolve.map((e) => ({ ...e, how: tr(e.how) })),
      breed: fd.breed,
    };
  }
  const family = d.family.map((n) => (n.how ? { ...n, how: tr(n.how) } : n));
  fs.writeFileSync(path.join(outDir, "species", `${sp.id}.json`), JSON.stringify({ id: sp.id, strings, family, forms }));
}

console.log(`${species.length} espèces, ${rows} façons d'obtenir, ${games.length} jeux → ${path.relative(root, outDir)}`);
console.log(`non traduits : ${untranslated.forms.size} formes, ${untranslated.how.size} évolutions, ${untranslated.places.size} lieux`);
if (process.env.VERBOSE) {
  for (const [k, v] of Object.entries(untranslated)) console.log(`\n# ${k}\n${[...v].join(" | ")}`);
}
