import { GLOSSARY } from "../../glossary";

/**
 * Bulles « i » du module Showdown / Smogon, ajoutées au glossaire commun
 * (utilisables partout avec `<Tip term="showdown" />` une fois ce module importé).
 */
Object.assign(GLOSSARY, {
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
  showdownSet: {
    title: "Set",
    text: "Une « fiche » de Pokémon prêt à combattre : espèce, objet tenu, talent, nature, répartition des EV (et parfois des IV), et 4 attaques. Appliquer un set remplace ces champs ; le dresseur d'origine, le lieu de rencontre et le PID sont gardés.",
  },
  showdownLang: {
    title: "Langue des noms",
    text: "Showdown n'accepte que les noms anglais (Garchomp, Earthquake, Choice Scarf…). Choisis « Anglais » pour coller le texte sur Showdown, « Français » pour le partager avec des joueurs francophones ou le garder pour toi. À l'import, Kaleido reconnaît les deux tout seul.",
  },
});
