import type { CompatMode, ItemSettings, RandomizerSettings, StaticSettings } from "../types";

/**
 * Libellés des réglages du randomizer, partagés entre ses onglets, son résumé et la fenêtre
 * « Paramètres de randomisation » de la bibliothèque (réglages relus dans une ROM générée).
 */

export type Opt<T extends string> = { value: T; label: string; hint?: string };

export const STARTER_OPTS: Opt<RandomizerSettings["starters"]>[] = [
  { value: "unchanged", label: "Inchangés" },
  { value: "random", label: "Aléatoires" },
  { value: "three_stage", label: "Trio évolutif", hint: "Pokémon de base avec deux évolutions" },
  { value: "triangle", label: "Plante · Feu · Eau", hint: "Trio évolutif qui garde le triangle des types" },
  { value: "custom", label: "Je choisis", hint: "Tape le nom des trois Pokémon de ton choix" },
];
export const WILD_OPTS: Opt<RandomizerSettings["wild"]>[] = [
  { value: "unchanged", label: "Inchangés" },
  { value: "random", label: "Totalement aléatoires" },
  { value: "area", label: "Par zone", hint: "Dans une zone, chaque espèce est remplacée par une même nouvelle espèce" },
  { value: "global", label: "Global", hint: "Une espèce devient la même partout dans le jeu" },
];
export const TRAINER_OPTS: Opt<RandomizerSettings["trainers"]>[] = [
  { value: "unchanged", label: "Inchangés" },
  { value: "random", label: "Aléatoires" },
  { value: "type_themed", label: "Thématiques", hint: "Chaque dresseur garde un type dominant (champions compris)" },
];
export const STATS_OPTS: Opt<RandomizerSettings["stats"]>[] = [
  { value: "unchanged", label: "Statistiques inchangées" },
  { value: "shuffle", label: "Mélangées", hint: "Les 6 statistiques sont permutées, le total ne change pas" },
  { value: "random", label: "Redistribuées", hint: "Nouvelle répartition du même total" },
];
export const CATCH_OPTS: Opt<RandomizerSettings["catchRate"]>[] = [
  { value: "unchanged", label: "Normale" },
  { value: "doubled", label: "Facile (×2)" },
  { value: "max", label: "Garantie", hint: "Taux de capture maximal pour toutes les espèces" },
];
export const TM_COMPAT_OPTS: Opt<CompatMode>[] = [
  { value: "unchanged", label: "Normale" },
  { value: "random_prefer_type", label: "Aléatoire (selon le type)" },
  { value: "random", label: "Aléatoire" },
  { value: "full", label: "Toutes les CT pour tous" },
];
export const TUTOR_COMPAT_OPTS: Opt<CompatMode>[] = [
  { value: "unchanged", label: "Normale" },
  { value: "random_prefer_type", label: "Aléatoire (selon le type)" },
  { value: "full", label: "Tout pour tous" },
];
export const FIELD_OPTS: Opt<ItemSettings["fieldItems"]>[] = [
  { value: "unchanged", label: "Inchangés" },
  { value: "shuffle", label: "Mélangés", hint: "Les mêmes objets, à d'autres endroits" },
  { value: "random", label: "Aléatoires" },
  { value: "random_even", label: "Aléatoires équilibrés", hint: "Chaque objet sort une fois avant toute répétition" },
];
export const SHOP_OPTS: Opt<ItemSettings["shops"]>[] = [
  { value: "unchanged", label: "Inchangées" },
  { value: "shuffle", label: "Mélangées" },
  { value: "random", label: "Aléatoires", hint: "Les comptoirs principaux et les boutiques de CT ne changent pas" },
];
export const STATIC_OPTS: Opt<StaticSettings["mode"]>[] = [
  { value: "unchanged", label: "Inchangés" },
  { value: "swap_legendaries", label: "Légendaire contre légendaire", hint: "Un légendaire devient un autre légendaire, un Pokémon ordinaire un autre ordinaire" },
  { value: "similar_strength", label: "Puissance similaire" },
  { value: "random", label: "Aléatoires" },
];
export const TRADE_OPTS: Opt<StaticSettings["trades"]>[] = [
  { value: "unchanged", label: "Inchangés" },
  { value: "given", label: "Pokémon reçu aléatoire", hint: "Le Pokémon demandé reste le même" },
  { value: "given_and_requested", label: "Reçu et demandé aléatoires" },
];
const CTR_OUTPUT_LABELS: Record<RandomizerSettings["ctrOutput"], string> = {
  layered_fs: "mod LayeredFS",
  rom3ds: "ROM .3ds complète",
  both: "mod LayeredFS et ROM .3ds",
};

export const lab = <T extends string>(opts: Opt<T>[], v: T) => opts.find((o) => o.value === v)?.label ?? v;

export function levelLabel(p: number) {
  return p === 100 ? "inchangés" : `${p > 100 ? "+" : ""}${p - 100} %`;
}

/** 3DS, même calcul que le moteur (data/shiny_ctr.rs) : N tirages de PID, N exprimable
 *  par `mov r0, #N` en ARM (un octet tourné d'un nombre pair de bits). */
const armImmediate = (v: number) => {
  for (let rot = 0; rot < 16; rot++) {
    const r = 2 * rot;
    const imm = r === 0 ? v : ((v << r) | (v >>> (32 - r))) >>> 0;
    if (imm <= 0xff) return true;
  }
  return false;
};
const ctrRerolls = (odds: number): number | "always" | null => {
  if (odds <= 1) return "always";
  if (odds >= 4096) return null;
  const n = Math.min(0x4000, Math.max(1, Math.round(Math.log(1 - 1 / odds) / Math.log(4095 / 4096))));
  for (let d = 0; d <= n; d++) {
    for (const v of [n - d, n + d]) if (v >= 1 && v <= 0x4000 && armImmediate(v)) return v === 1 ? null : v;
  }
  return null;
};
const ctrShinyLabel = (odds: number) => {
  const n = ctrRerolls(odds);
  if (n === "always") return "tous les Pokémon";
  if (n === null) return "1 / 4096 (normal)";
  return `1 / ${Math.max(1, Math.round(1 / (1 - Math.pow(4095 / 4096, n))))}`;
};

/** Même calcul que le moteur : seuil arrondi entre 1 et 255 (sur 65 536). */
export function shinyLabel(odds: number, isCtr: boolean) {
  const n = Number(odds) || 8192;
  if (isCtr) return ctrShinyLabel(n);
  if (n <= 1) return "tous les Pokémon";
  const threshold = Math.min(255, Math.max(1, Math.round(65536 / n)));
  return `1 / ${Math.floor(65536 / threshold)}`;
}
/** Le taux sera-t-il vraiment modifié ? (3DS : rien sous 1 / 4 096) */
export function shinyChanged(odds: number, isCtr: boolean) {
  const n = Number(odds) || 8192;
  return isCtr ? ctrRerolls(n) !== null : n !== 8192;
}

export type TabId = "general" | "pokemon" | "starters" | "wild" | "trainers" | "moves" | "items" | "statics" | "shiny";

export interface SettingRow {
  tab: TabId;
  label: string;
  value: string;
  /** Différent des réglages de référence. */
  changed: boolean;
}

export interface DescribeContext {
  isCtr: boolean;
  /** Pokémon X / Y : second trio de starters (Professeur Platane). */
  isXy: boolean;
  /** Noms des starters choisis à la main (mode « Je choisis »). */
  starterNames: string[];
  kantoNames: string[];
}

/** Tous les réglages en français courant, chacun marqué s'il diffère de `d`. */
export function describeSettings(s: RandomizerSettings, d: RandomizerSettings, ctx: DescribeContext): SettingRow[] {
  const out: SettingRow[] = [];
  const add = (tab: TabId, changed: boolean, label: string, value: string) => out.push({ tab, label, value, changed });
  const flag = (tab: TabId, label: string, v: boolean, dv: boolean) => add(tab, v !== dv, label, v ? "oui" : "non");
  const opt = <T extends string>(tab: TabId, label: string, opts: Opt<T>[], v: T, dv: T, extra = "") =>
    add(tab, v !== dv, label, lab(opts, v).toLowerCase() + extra);
  const pct = (p: number) => `${p >= 0 ? "+" : ""}${p} %`;
  const names = (list: string[]) => {
    const n = list.filter((x) => x.trim()).join(", ");
    return n ? ` (${n})` : "";
  };

  if (ctx.isCtr) add("general", s.ctrOutput !== d.ctrOutput, "Sortie", CTR_OUTPUT_LABELS[s.ctrOutput] ?? s.ctrOutput);

  add("pokemon", s.stats !== d.stats, "Statistiques", s.stats === "unchanged" ? "inchangées" : lab(STATS_OPTS, s.stats).toLowerCase());
  flag("pokemon", "Types aléatoires", s.randomTypes, d.randomTypes);
  flag("pokemon", "Talents aléatoires", s.randomAbilities, d.randomAbilities);
  flag("pokemon", "Sans légendaires", s.noLegendaries, d.noLegendaries);
  flag("pokemon", "Évolutions sans échange", s.easyEvolutions, d.easyEvolutions);
  flag("pokemon", "Attaques apprises aléatoires", s.randomMovesets, d.randomMovesets);
  opt("pokemon", "Capture", CATCH_OPTS, s.catchRate, d.catchRate);

  opt("starters", "Starters", STARTER_OPTS, s.starters, d.starters, s.starters === "custom" ? names(ctx.starterNames) : "");
  if (ctx.isXy) {
    const kanto = s.kantoStarters ?? "unchanged";
    opt("starters", "Pokémon de Kanto", STARTER_OPTS, kanto, "unchanged", kanto === "custom" ? names(ctx.kantoNames) : "");
  }

  opt("wild", "Pokémon sauvages", WILD_OPTS, s.wild, d.wild);
  flag("wild", "Sauvages de puissance similaire", s.wildSimilarStrength, d.wildSimilarStrength);
  add("wild", s.wildLevelPercent !== d.wildLevelPercent, "Niveaux des sauvages", levelLabel(s.wildLevelPercent));

  opt("trainers", "Dresseurs", TRAINER_OPTS, s.trainers, d.trainers);
  flag("trainers", "Dresseurs de puissance similaire", s.trainersSimilarStrength, d.trainersSimilarStrength);
  add("trainers", s.trainerLevelPercent !== d.trainerLevelPercent, "Niveaux des dresseurs", levelLabel(s.trainerLevelPercent));
  flag("trainers", "Pokémon des dresseurs évolués", s.trainerEvolutions, d.trainerEvolutions);
  flag("trainers", "IV des dresseurs au maximum", s.trainerMaxIvs, d.trainerMaxIvs);

  const m = s.moves;
  const dm = d.moves;
  flag("moves", "CT aléatoires", m.randomTms, dm.randomTms);
  flag("moves", "Maîtres des capacités aléatoires", m.randomTutors, dm.randomTutors);
  flag("moves", "Garder les attaques de terrain", m.keepFieldMoves, dm.keepFieldMoves);
  flag("moves", "Sans Sonicboom / Draco-Rage", m.noGameBreaking, dm.noGameBreaking);
  add("moves", m.goodDamagingPercent !== dm.goodDamagingPercent, "Attaques offensives garanties", `${m.goodDamagingPercent} %`);
  opt("moves", "Compatibilité CT", TM_COMPAT_OPTS, m.tmCompat, dm.tmCompat);
  opt("moves", "Maîtres des capacités", TUTOR_COMPAT_OPTS, m.tutorCompat, dm.tutorCompat);
  flag("moves", "Toutes les CS pour tous", m.fullHmCompat, dm.fullHmCompat);
  flag("moves", "Les évolutions héritent des CT", m.followEvolutions, dm.followEvolutions);
  flag("moves", "Garder les CT des attaques apprises", m.levelupSanity, dm.levelupSanity);

  const it = s.items;
  const di = d.items;
  opt("items", "Objets au sol", FIELD_OPTS, it.fieldItems, di.fieldItems);
  opt("items", "Boutiques", SHOP_OPTS, it.shops, di.shops);
  flag("items", "Pas d'objets inutiles", it.banBadFieldItems, di.banBadFieldItems);
  flag("items", "Pas d'objets inutiles en boutique", it.banBadShopItems, di.banBadShopItems);
  flag("items", "Pas d'objets ordinaires en boutique", it.banRegularShopItems, di.banRegularShopItems);
  flag("items", "Pierres d'évolution en vente", it.guaranteeEvolutionItems, di.guaranteeEvolutionItems);
  flag("items", "Objets X en vente", it.guaranteeXItems, di.guaranteeXItems);
  flag("items", "Pas d'objets trop forts en boutique", it.banOpShopItems, di.banOpShopItems);
  flag("items", "Sans Super Bonbon", it.noRareCandy, di.noRareCandy);
  flag("items", "Sans Master Ball", it.noMasterBall, di.noMasterBall);

  const st = s.statics;
  const ds = d.statics;
  opt("statics", "Pokémon fixes et dons", STATIC_OPTS, st.mode, ds.mode);
  add("statics", st.levelModifier !== ds.levelModifier, "Niveaux des Pokémon fixes", pct(st.levelModifier));
  opt("statics", "Échanges", TRADE_OPTS, st.trades, ds.trades);
  flag("statics", "Objets tenus aléatoires (échanges)", st.tradeRandomItems, ds.tradeRandomItems);
  flag("statics", "IV aléatoires (échanges)", st.tradeRandomIvs, ds.tradeRandomIvs);

  add("shiny", shinyChanged(s.shinyOdds, ctx.isCtr), "Chromatiques", shinyLabel(s.shinyOdds, ctx.isCtr));
  return out;
}
