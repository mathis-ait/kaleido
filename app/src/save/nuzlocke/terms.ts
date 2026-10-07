import type { GlossaryEntry } from "../../glossary";
import { RULES } from "./types";

/** Bulles « i » du Nuzlocke (glossaire central, préfixe « nuzlocke. ») : une par règle, puis les notions de suivi. */
export const NUZLOCKE_TERMS: Record<string, GlossaryEntry> = {
  unassigned: {
    title: "Lieux à rattacher",
    text: "Kaleido regroupe les rencontres par nom de lieu. Certains lieux ne correspondent à aucune route de la ROM (étage d'une grotte, Parc Safari, lieu sans herbes…). Choisis une fois la route à laquelle ils appartiennent, ou « Pas une route » pour un cadeau ou une rencontre fixe.",
  },
  ...Object.fromEntries(RULES.map((r) => [r.id, { title: r.label, text: r.tip }])),

  nuzlocke: {
    title: "Nuzlocke",
    text: "Un défi qui rend le jeu plus difficile : on ne capture que le premier Pokémon de chaque route, et un Pokémon K.O. est considéré comme mort (il ne peut plus être utilisé). Kaleido suit ta partie à partir de la sauvegarde.",
  },
  linkedRom: {
    title: "ROM liée",
    text: "La ROM avec laquelle tu joues cette partie. Kaleido y lit les rencontres de chaque route, les familles d'évolution et les équipes des champions : après une randomisation, ce sont donc bien les tiens. Le lien est gardé dans un petit fichier à côté de la sauvegarde.",
  },
  seed: {
    title: "Seed",
    text: "Le nombre qui a servi à randomiser la ROM avec Kaleido. La même seed avec les mêmes réglages redonne exactement la même ROM.",
  },
  badges: {
    title: "Badges",
    text: "Lus dans la sauvegarde. Tu peux forcer une valeur si besoin : le niveau maximum suit le nombre de badges.",
  },
  trials: {
    title: "Épreuves",
    text: "Capitaines et doyens battus, déduits des îles terminées (Grands Duels) enregistrées dans la sauvegarde. Tu peux forcer une valeur si besoin : le niveau maximum suit ce nombre.",
  },
  firstEncounter: {
    title: "Première rencontre",
    text: "Kaleido range chaque Pokémon de la sauvegarde sur la route où il a été rencontré (lieu de rencontre). Le starter, les œufs, les échanges et les cadeaux ne comptent pas. Marque une route « ratée » si le premier Pokémon s'est enfui ou est tombé K.O.",
  },
  missed: {
    title: "Route ratée",
    text: "Le premier Pokémon de la route s'est enfui ou est tombé K.O. : plus de capture possible ici. Kaleido ne peut pas le savoir seul, c'est à toi de la marquer.",
  },
  dupe: {
    title: "Doublon",
    text: "Espèce déjà capturée, ou de la même famille d'évolution (familles lues dans la ROM). Avec la clause doublons, elle ne compte pas : la route reste ouverte pour la rencontre suivante.",
  },
  shinyBonus: {
    title: "Chromatique en plus",
    text: "Pokémon chromatique capturé sur une route déjà faite. La clause chromatique l'autorise : il ne compte pas comme la capture de la route.",
  },
  extra: {
    title: "Capture de trop",
    text: "Deuxième capture sur une route qui en avait déjà une, sans clause pour la justifier. Elle apparaît dans les infractions.",
  },
  others: {
    title: "Hors routes",
    text: "Pokémon obtenus autrement qu'en capture sur une route : starter, œufs, échanges, cadeaux et Pokémon fixes, ou lieu de rencontre que Kaleido ne relie à aucune route. Ils ne comptent pour aucune route.",
  },
  graveyard: {
    title: "Cimetière",
    text: "Les Pokémon morts : K.O. dans l'équipe (PV à 0), rangés dans la boîte « Cimetière », ou marqués à la main. Une boîte est reconnue toute seule si son nom contient RIP, Cimetière ou Morts ; tu peux aussi la choisir dans l'onglet Règles.",
  },
  ace: {
    title: "Ace",
    text: "Le Pokémon le plus fort (« ace ») d'un champion donne le niveau maximum à ne pas dépasser avant de l'affronter.",
  },
  unverified: {
    title: "Adversaire à vérifier",
    text: "La classe de dresseur lue dans la ROM n'est pas celle attendue pour ce champion : la ROM a peut-être été modifiée autrement qu'avec Kaleido. Le niveau affiché reste celui de la ROM.",
  },
  violations: {
    title: "Infractions",
    text: "Les règles enfreintes repérées dans la sauvegarde. Rouge : règle enfreinte · orange : à vérifier · bleu : pour information. Les règles « pas de soins en combat » et « mode Set » ne peuvent pas être vérifiées.",
  },
  reminder: {
    title: "Rappel",
    text: "Règle que la sauvegarde ne permet pas de vérifier : Kaleido l'affiche pour mémoire, à toi de la respecter.",
  },
};
