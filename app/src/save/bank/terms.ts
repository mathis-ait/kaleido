import type { GlossaryEntry } from "../../glossary";

/**
 * Bulles « i » de la Banque Kaleido (glossaire central, préfixe « bank. »).
 * Les termes généraux (soigneur, chromatique, lieu de rencontre…) restent dans le glossaire central.
 */
export const BANK_TERMS: Record<string, GlossaryEntry> = {
  bank: {
    title: "Banque Kaleido",
    text: "Un PC commun à toutes tes sauvegardes, rangé sur ton ordinateur. Chaque Pokémon y garde son format d'origine (fichiers .pk4 à .pk7, comme ceux de PKHeX) et peut ensuite partir vers n'importe quelle sauvegarde compatible.",
  },
  direction: {
    title: "Sens des transferts",
    text: "Comme dans les vrais jeux (Poké Transfert, Poké Transporteur, Banque Pokémon), un Pokémon ne va que vers une génération égale ou plus récente : Gen 4, puis 5, 6 et 7. Retourner vers un jeu plus ancien est impossible : le Pokémon reste alors dans la banque.",
  },
  changes: {
    title: "Ce qui change au transfert",
    text: "Gen 4 vers 5 : lieu de rencontre « Poké Transfert », CS oubliées, bonheur remis à 70. Gen 5 vers 6 : objet tenu laissé derrière, bonheur de base, rubans réorganisés. Gen 6 vers 7 : souvenirs de voyage effacés. Dès la Gen 6, le dresseur de la sauvegarde devient son soigneur. IV, EV, nature, attaques, surnom, dresseur d'origine et chromatique sont gardés.",
  },
  format: {
    title: "Format PK4 à PK7",
    text: "Chaque génération range ses Pokémon à sa façon : PK4 (Diamant, Perle, Platine, HGSS), PK5 (Noir/Blanc 1 et 2), PK6 (X/Y, ROSA), PK7 (Soleil/Lune, Ultra). Les filtres PK4 à PK7 ne gardent que les Pokémon rangés dans ce format. Kaleido convertit automatiquement au format de la sauvegarde.",
  },
  trash: {
    title: "Corbeille de la banque",
    text: "Un Pokémon supprimé ou envoyé dans une sauvegarde n'est jamais effacé : son fichier part dans le dossier « corbeille » de la banque. Tu peux le réimporter à tout moment.",
  },
  drag: {
    title: "Entre la banque et la sauvegarde",
    text: "Glisse un Pokémon de la banque vers la sauvegarde pour l'y envoyer (converti si besoin), ou l'inverse pour le ranger dans la banque. Maj + glisser fait une copie : l'original reste en place. Ctrl+Z défait le changement dans la sauvegarde ; la banque, elle, est enregistrée tout de suite.",
  },
};
