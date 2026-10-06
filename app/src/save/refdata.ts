/** Données de référence de l'éditeur de sauvegardes (identifiants de PKHeX). */

export const STAT_LABELS = ["PV", "Attaque", "Défense", "Atq. Spé.", "Déf. Spé.", "Vitesse"];
export const STAT_SHORT = ["PV", "Att", "Déf", "AtS", "DéS", "Vit"];

export const NATURES = [
  "Hardi", "Solo", "Brave", "Rigide", "Mauvais", "Assuré", "Docile", "Relax", "Malin", "Lâche",
  "Timide", "Pressé", "Sérieux", "Jovial", "Naïf", "Modeste", "Doux", "Discret", "Pudique", "Foufou",
  "Calme", "Gentil", "Malpoli", "Prudent", "Bizarre",
];

/** Ordre des statistiques des natures (Att, Déf, Vit, AtS, DéS) → index Kaleido. */
const NATURE_STAT = [1, 2, 5, 3, 4];

/** Statistique augmentée et baissée (index Kaleido), `null` pour une nature neutre. */
export function natureEffect(n: number): { up: number; down: number } | null {
  const up = Math.floor(n / 5);
  const down = n % 5;
  return up === down ? null : { up: NATURE_STAT[up], down: NATURE_STAT[down] };
}

export function natureLabel(n: number) {
  const e = natureEffect(n);
  return e ? `${NATURES[n]} (+${STAT_SHORT[e.up]} −${STAT_SHORT[e.down]})` : `${NATURES[n]} (neutre)`;
}

export interface Ball {
  id: number;
  name: string;
  /** Première génération où la Ball existe. */
  since: number;
  /** Dernière génération (Parc Ball). */
  until?: number;
  color: string;
}

export const BALLS: Ball[] = [
  { id: 1, name: "Master Ball", since: 3, color: "#8e44ad" },
  { id: 2, name: "Hyper Ball", since: 3, color: "#f1c40f" },
  { id: 3, name: "Super Ball", since: 3, color: "#2f80ed" },
  { id: 4, name: "Poké Ball", since: 3, color: "#e74c3c" },
  { id: 5, name: "Safari Ball", since: 3, color: "#7d8f3a" },
  { id: 6, name: "Filet Ball", since: 3, color: "#16a085" },
  { id: 7, name: "Scuba Ball", since: 3, color: "#3498db" },
  { id: 8, name: "Faiblo Ball", since: 3, color: "#27ae60" },
  { id: 9, name: "Bis Ball", since: 3, color: "#c0392b" },
  { id: 10, name: "Chrono Ball", since: 3, color: "#e67e22" },
  { id: 11, name: "Luxe Ball", since: 3, color: "#2c3e50" },
  { id: 12, name: "Honor Ball", since: 3, color: "#ecf0f1" },
  { id: 13, name: "Sombre Ball", since: 4, color: "#34495e" },
  { id: 14, name: "Soin Ball", since: 4, color: "#ff8fb1" },
  { id: 15, name: "Rapide Ball", since: 4, color: "#f39c12" },
  { id: 16, name: "Mémoire Ball", since: 3, color: "#c0392b" },
  { id: 17, name: "Speed Ball", since: 4, color: "#e67e22" },
  { id: 18, name: "Niveau Ball", since: 4, color: "#e74c3c" },
  { id: 19, name: "Appât Ball", since: 4, color: "#2980b9" },
  { id: 20, name: "Masse Ball", since: 4, color: "#7f8c8d" },
  { id: 21, name: "Love Ball", since: 4, color: "#ff6fa8" },
  { id: 22, name: "Copain Ball", since: 4, color: "#27ae60" },
  { id: 23, name: "Lune Ball", since: 4, color: "#2c3e8f" },
  { id: 24, name: "Compet'Ball", since: 4, color: "#d35400" },
  { id: 25, name: "Parc Ball", since: 4, until: 4, color: "#f1c40f" },
  { id: 26, name: "Rêve Ball", since: 5, color: "#ff8fd8" },
  { id: 27, name: "Ultra Ball (Chimères)", since: 7, color: "#2d3cbe" },
];

export const LANGUAGES = [
  { id: 1, name: "Japonais", code: "JPN" },
  { id: 2, name: "Anglais", code: "ENG" },
  { id: 3, name: "Français", code: "FRA" },
  { id: 4, name: "Italien", code: "ITA" },
  { id: 5, name: "Allemand", code: "GER" },
  { id: 7, name: "Espagnol", code: "ESP" },
  { id: 8, name: "Coréen", code: "KOR" },
  { id: 9, name: "Chinois simplifié", code: "CHS", since: 7 },
  { id: 10, name: "Chinois traditionnel", code: "CHT", since: 7 },
];

/** Jeux d'origine (`GameVersion` de PKHeX) jusqu'à la Gen 7. */
export const VERSIONS = [
  { id: 1, name: "Saphir", gen: 3 },
  { id: 2, name: "Rubis", gen: 3 },
  { id: 3, name: "Émeraude", gen: 3 },
  { id: 4, name: "Rouge Feu", gen: 3 },
  { id: 5, name: "Vert Feuille", gen: 3 },
  { id: 15, name: "Colosseum / XD", gen: 3 },
  { id: 10, name: "Diamant", gen: 4 },
  { id: 11, name: "Perle", gen: 4 },
  { id: 12, name: "Platine", gen: 4 },
  { id: 7, name: "Or HeartGold", gen: 4 },
  { id: 8, name: "Argent SoulSilver", gen: 4 },
  { id: 21, name: "Noire", gen: 5 },
  { id: 20, name: "Blanche", gen: 5 },
  { id: 23, name: "Noire 2", gen: 5 },
  { id: 22, name: "Blanche 2", gen: 5 },
  { id: 24, name: "X", gen: 6 },
  { id: 25, name: "Y", gen: 6 },
  { id: 27, name: "Rubis Oméga", gen: 6 },
  { id: 26, name: "Saphir Alpha", gen: 6 },
  { id: 30, name: "Soleil", gen: 7 },
  { id: 31, name: "Lune", gen: 7 },
  { id: 32, name: "Ultra-Soleil", gen: 7 },
  { id: 33, name: "Ultra-Lune", gen: 7 },
  { id: 35, name: "Rouge (Console virtuelle)", gen: 7 },
  { id: 36, name: "Vert (Console virtuelle)", gen: 7 },
  { id: 37, name: "Bleu (Console virtuelle)", gen: 7 },
  { id: 38, name: "Jaune (Console virtuelle)", gen: 7 },
  { id: 39, name: "Or (Console virtuelle)", gen: 7 },
  { id: 40, name: "Argent (Console virtuelle)", gen: 7 },
  { id: 41, name: "Cristal (Console virtuelle)", gen: 7 },
];

export const MARKS = ["●", "▲", "■", "♥", "★", "◆"];

export const POUCH_LABELS: Record<string, string> = {
  items: "Objets",
  key_items: "Objets rares",
  tm_hm: "CT & CS",
  medicine: "Soins",
  berries: "Baies",
  balls: "Poké Balls",
  battle_items: "Objets de combat",
  mail: "Lettres",
  z_crystals: "Cristaux Z",
  roto_powers: "Rotom-Pouvoirs",
};

export const POUCH_COLORS: Record<string, string> = {
  items: "#ff5d8f",
  key_items: "#8b5cf6",
  tm_hm: "#22c55e",
  medicine: "#f97316",
  berries: "#16a34a",
  balls: "#ef4444",
  battle_items: "#3b82f6",
  mail: "#eab308",
  z_crystals: "#06b6d4",
  roto_powers: "#f43f5e",
};

export const genderSymbol = (g: string) => (g === "male" ? "♂" : g === "female" ? "♀" : "");

export const pad = (n: number, w = 2) => String(n).padStart(w, "0");

/** Clés de couleur des types, dans l'ordre des identifiants PKHeX (0 Normal … 17 Fée). */
export const TYPE_KEYS = [
  "normal", "fighting", "flying", "poison", "ground", "rock", "bug", "ghost", "steel",
  "fire", "water", "grass", "electric", "psychic", "ice", "dragon", "dark", "fairy",
] as const;

/** Type de Puissance Cachée d'après les IV (ordre Kaleido : PV, Att, Déf, AtS, DéS, Vit). */
export function hiddenPowerType(ivs: number[]) {
  // Ordre de la formule : PV, Att, Déf, Vit, AtS, DéS.
  const order = [ivs[0], ivs[1], ivs[2], ivs[5], ivs[3], ivs[4]];
  const t = Math.floor((order.reduce((acc, iv, i) => acc + ((iv & 1) << i), 0) * 15) / 63);
  return t + 1; // de Combat (1) à Ténèbres (16)
}

/**
 * IV donnant la Puissance Cachée de type `type` (1 Combat … 16 Ténèbres) en changeant le moins
 * d'IV possible, d'un point chacune (parité seulement) ; à égalité, on garde les IV les plus hautes.
 */
export function ivsForHiddenPower(ivs: number[], type: number): number[] {
  // Ordre de la formule : PV, Att, Déf, Vit, AtS, DéS → index Kaleido.
  const order = [0, 1, 2, 5, 3, 4];
  let best: number[] | null = null;
  let bestCost: [number, number] = [Infinity, Infinity];
  for (let bits = 0; bits < 64; bits++) {
    if (Math.floor((bits * 15) / 63) + 1 !== type) continue;
    const out = [...ivs];
    let changed = 0;
    let lost = 0;
    order.forEach((k, i) => {
      const want = (bits >> i) & 1;
      if ((out[k] & 1) === want) return;
      changed++;
      if (want) out[k] += 1;
      else {
        out[k] -= 1;
        lost++;
      }
    });
    if (changed < bestCost[0] || (changed === bestCost[0] && lost < bestCost[1])) {
      best = out;
      bestCost = [changed, lost];
    }
  }
  return best ?? ivs;
}

export function genderLabel(g: string) {
  return g === "male" ? "Mâle" : g === "female" ? "Femelle" : "Asexué";
}
