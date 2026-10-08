import type { GlossaryEntry } from "../../glossary";

/** Bulles « i » du module Showdown / Smogon (fusionnées sans préfixe dans le glossaire central). */
export const SHOWDOWN_TERMS: Record<string, GlossaryEntry> = {
  showdown: {
    title: "Pokémon Showdown",
    text: "Simulateur de combats Pokémon en ligne (pokemonshowdown.com). Ses équipes s'échangent sous forme de texte : « Carchacrok @ Mouchoir Choix », puis talent, EV, nature et attaques. Kaleido lit ce texte (en anglais ou en français) et crée les Pokémon dans ta sauvegarde, ou fait l'inverse pour jouer tes Pokémon sur Showdown.",
  },
  smogon: {
    title: "Smogon",
    text: "Communauté qui organise les règles du jeu compétitif (sur Showdown) et publie des analyses : pour chaque Pokémon, des « sets » conseillés (attaques, objet, talent, nature, EV). Kaleido les télécharge une fois puis les garde pour fonctionner hors ligne.",
  },
  smogonFormat: {
    title: "Formats et paliers",
    text: "Smogon classe les Pokémon par palier selon leur puissance : Ubers (légendaires très forts), OU (OverUsed, le format principal), puis UU, RU, NU, PU, ZU pour les moins utilisés. LC (Little Cup) se joue au niveau 5 avec des Pokémon non évolués ; VGC et Doubles sont des combats en double, niveau 50.",
  },
  smogonTeams: {
    title: "Équipes stratégiques",
    text: "Équipes complètes publiées par Smogon University comme exemples pour chaque format : six Pokémon qui se complètent (attaquants, défenseurs, soutiens). Kaleido les récupère via crob.at et les crée dans ta sauvegarde, avec objets, talents, natures, EV et attaques.",
  },
  showdownSet: {
    title: "Set",
    text: "Une « fiche » de Pokémon prêt à combattre : espèce, objet tenu, talent, nature, répartition des EV (et parfois des IV), et 4 attaques. Appliquer un set remplace ces champs ; le dresseur d'origine, le lieu de rencontre et le PID sont gardés quand c'est possible, puis le Pokémon passe par « Rendre légal » (sauf avec « Importer tel quel »).",
  },
  importAsIs: {
    title: "Importer tel quel",
    text: "Par défaut, chaque Pokémon importé passe par « Rendre légal » : Kaleido choisit une rencontre possible dans ce jeu (capture, œuf…) et ajuste ce qu'il faut (lieu, Ball, PID…), en gardant au mieux le set. Le rapport dit, Pokémon par Pokémon, ce qui a changé. Activé : le set est recopié sans vérification, comme avant (le Pokémon peut être illégal).",
  },
  showdownLang: {
    title: "Langue des noms",
    text: "Showdown n'accepte que les noms anglais (Garchomp, Earthquake, Choice Scarf…). Choisis « Anglais » pour coller le texte sur Showdown, « Français » pour le partager avec des joueurs francophones ou le garder pour toi. À l'import, Kaleido reconnaît les deux tout seul.",
  },
};
