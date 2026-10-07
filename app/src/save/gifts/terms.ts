/**
 * Bulles « i » propres à la page « Cadeaux mystère », fusionnées dans le glossaire central
 * sous le préfixe « gifts. » (`<Tip term="gifts.card" />`). Les termes généraux (rencontre
 * fatidique, verrou chromatique, « Seulement ce jeu »…) restent dans `glossary.ts`.
 */
export const GIFT_TERMS = {
  mysteryGift: {
    title: "Cadeau mystère",
    text: "Pokémon ou objet distribué par Nintendo et The Pokémon Company lors d'événements (films, tournois, anniversaires…). Le jeu le reçoit via le menu « Cadeau Mystère » puis le livreur du Centre Pokémon. La base vient de PKHeX : plus de 3 000 cartes des Gen 4 à 7.",
  },
  card: {
    title: "Carte cadeau (PCD, PGT, PGF, WC6, WC7)",
    text: "Fichier qui décrit le cadeau. PCD / PGT : Gen 4 (le PGT est le cadeau seul, sans la carte affichée). PGF : Gen 5. WC6 : X/Y, Rubis Oméga/Saphir Alpha. WC7 : Soleil/Lune, Ultra-Soleil/Ultra-Lune. Les « full » (.wc6full, .wc7full) contiennent aussi les restrictions de version et de langue de la distribution.",
  },
  distribution: {
    title: "Mode de distribution",
    text: "Selon l'événement, la carte arrivait par Internet (Wi-Fi), avec un code de série (boîte de jeu, magasin, carte promo), ou en local (sans fil, sur place). Pour un même Pokémon, chaque pays ou chaque langue avait souvent sa propre carte : c'est pourquoi la base contient des doublons.",
  },
  perfectIvs: {
    title: "IV garantis",
    text: "Les cartes récentes fixent certains IV, ou garantissent un nombre d'IV à 31 tirés au hasard parmi les six (souvent 3 pour les Pokémon fabuleux).",
  },
  ot: {
    title: "Dresseur d'origine de la carte",
    text: "La plupart des Pokémon d'événement gardent le nom et l'ID de l'événement (« GF », « MT. Tensei », « WORLD12 »…). Certains prennent ceux de ton dresseur : la carte l'indique par « Le tien ».",
  },
  generation: {
    title: "Même génération seulement",
    text: "Un cadeau ne peut être reçu que par un jeu de sa génération. Un Pokémon d'une génération plus ancienne arrive dans les jeux récents par transfert (Poké Transfert, Poké Fret, Pokémon Bank), ce que Kaleido ne simule pas encore.",
  },
  receive: {
    title: "Ajouter à la sauvegarde",
    text: "Kaleido fait comme le livreur : le Pokémon est créé selon les règles de la carte (PID, IV, nature, talent…) avec ta langue et la date du jour, puis rangé dans la première case libre des boîtes. Les objets vont dans le sac. Une seule étape d'annulation (Ctrl+Z).",
  },
} as const;

/**
 * Teinte de chaque format de carte (table de données, comme les couleurs des types) :
 * pastille du format, fond de l'image et contour de la carte choisie. Seule définition,
 * partagée par la grille et la fiche.
 */
const FORMAT_TONES: Record<string, string> = {
  pcd: "#3b82f6",
  pgt: "#3b82f6",
  pgf: "#64748b",
  wc6: "#db2777",
  wc7: "#ea7a1a",
};

/** Style `--gift-tone` d'un format (« PCD », « WC7 »…), à poser sur l'élément qui l'utilise. */
export function giftTone(formatLabel: string): Record<string, string> {
  return { "--gift-tone": FORMAT_TONES[formatLabel.toLowerCase()] ?? "var(--text-dim)" };
}
