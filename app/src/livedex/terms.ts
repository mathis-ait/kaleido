import type { GlossaryEntry } from "../glossary";

/** Termes de la Living Dex (bulles « i »), préfixe `livedex.`. */
export const LIVEDEX_TERMS: Record<string, GlossaryEntry> = {
  livingDex: {
    title: "Living Dex",
    text: "Un Pokédex « vivant » : au lieu de cocher « vu » ou « capturé », on garde un exemplaire de chaque Pokémon dans ses boîtes. Kaleido lit tes sauvegardes et coche tout seul les Pokémon que tu possèdes vraiment.",
  },
  presets: {
    title: "Règles des cases",
    text: "Espèces : une case par Pokémon (1 025). Formes : aussi les formes régionales, liées au sexe, apparences et formes modifiables (1 365). Complétionniste : absolument tout, y compris les formes d'évènement, Méga-Évolutions et Gigamax (1 627).",
  },
  form: {
    title: "Forme",
    text: "Certains Pokémon existent sous plusieurs apparences : Raichu d'Alola, les 28 lettres de Zarbi, les motifs de Prismillon… Selon les règles choisies, une forme a sa propre case ou compte pour l'espèce.",
  },
  auto: {
    title: "Lu dans une sauvegarde",
    text: "Pokémon trouvé par Kaleido dans l'équipe ou les boîtes d'une de tes sauvegardes (ou dans la banque). Il se met à jour tout seul : s'il quitte la sauvegarde, sa case se vide.",
  },
  manual: {
    title: "Noté à la main",
    text: "Pour les jeux que Kaleido ne lit pas (Switch, Pokémon HOME, Pokémon GO, une vraie cartouche…), tu peux noter un Pokémon toi-même. Il reste jusqu'à ce que tu le supprimes.",
  },
  obtain: {
    title: "Où le trouver",
    text: "Toutes les façons connues d'obtenir ce Pokémon, jeu par jeu : rencontres sauvages, dons, échanges, raids, évènements… Tes jeux (ceux de la Bibliothèque) sont listés en premier. Données de PKHeX, via Pelagix.",
  },
  rank: {
    title: "Rang de collectionneur",
    text: "Il monte avec la part des cases remplies : Débutant, Apprenti (5 %), Collectionneur (15 %), Expert (30 %), Spécialiste (50 %), Maître (70 %), Champion (90 %) et Légende (100 %).",
  },
};
