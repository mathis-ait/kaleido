/**
 * Aides en bas d'écran : glyphes des boutons selon la disposition de la manette.
 * Lettres Nintendo par défaut (L / R, +), lettres Xbox si la manette l'annonce
 * (LB / RB, Menu). Les lettres A, B, X, Y gardent le même rôle dans les deux cas :
 * A joue, B revient, X inspecte, Y lance une nouvelle aventure.
 */

export type PadLayout = "nintendo" | "xbox";
export type HintAction = "move" | "accept" | "back" | "inspect" | "adventure" | "tabs" | "menu";

export function layoutOf(id: string | null | undefined): PadLayout {
  if (!id) return "nintendo";
  return /xbox|xinput|microsoft|045e/i.test(id) ? "xbox" : "nintendo";
}

export interface Glyph {
  /** Texte du glyphe. */
  text: string;
  /** Forme : rond (bouton de façade), pilule (gâchettes, Start), touche (clavier). */
  shape: "round" | "pill" | "key";
}

const PAD: Record<PadLayout, Record<HintAction, Glyph[]>> = {
  nintendo: {
    move: [{ text: "◀ ▶", shape: "pill" }],
    accept: [{ text: "A", shape: "round" }],
    back: [{ text: "B", shape: "round" }],
    inspect: [{ text: "X", shape: "round" }],
    adventure: [{ text: "Y", shape: "round" }],
    tabs: [
      { text: "L", shape: "pill" },
      { text: "R", shape: "pill" },
    ],
    menu: [{ text: "+", shape: "round" }],
  },
  xbox: {
    move: [{ text: "◀ ▶", shape: "pill" }],
    accept: [{ text: "A", shape: "round" }],
    back: [{ text: "B", shape: "round" }],
    inspect: [{ text: "X", shape: "round" }],
    adventure: [{ text: "Y", shape: "round" }],
    tabs: [
      { text: "LB", shape: "pill" },
      { text: "RB", shape: "pill" },
    ],
    menu: [{ text: "Menu", shape: "pill" }],
  },
};

const KEYBOARD: Record<HintAction, Glyph[]> = {
  move: [{ text: "◀ ▶", shape: "key" }],
  accept: [{ text: "Entrée", shape: "key" }],
  back: [{ text: "Échap", shape: "key" }],
  inspect: [{ text: "I", shape: "key" }],
  adventure: [{ text: "N", shape: "key" }],
  tabs: [
    { text: "Q", shape: "key" },
    { text: "E", shape: "key" },
  ],
  menu: [{ text: "M", shape: "key" }],
};

export function glyphs(action: HintAction, pad: boolean, layout: PadLayout): Glyph[] {
  return pad ? PAD[layout][action] : KEYBOARD[action];
}
