import type { GlossaryEntry } from "../glossary";

/** Bulles « i » de Nouvelle aventure (glossaire central, préfixe « adventure. »). */
export const ADVENTURE_TERMS: Record<string, GlossaryEntry> = {
  adventure: {
    title: "Nouvelle aventure",
    text: "Une partie randomisée en trois clics : le jeu, un preset, puis l'aperçu du début de partie. « Jouer » écrit la ROM (ou le mod 3DS) à côté du jeu d'origine, lance l'émulateur et ouvre le compagnon. Le jeu d'origine n'est jamais modifié.",
  },
  seed: {
    title: "Seed et code de partage",
    text: "La seed est le numéro du tirage : même jeu, même preset et même seed donnent exactement la même partie. Le code « KLD1-… » contient la seed et tous les réglages : envoie-le à un ami, il le colle ici et joue la même aventure que toi.",
  },
};
