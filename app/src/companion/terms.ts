import type { GlossaryEntry } from "../glossary";

/** Bulles « i » du compagnon de partie (glossaire central, préfixe « companion. »). */
export const COMPANION_TERMS: Record<string, GlossaryEntry> = {
  companion: {
    title: "Compagnon de partie",
    text: "Petite fenêtre à garder à côté de l'émulateur. À chaque sauvegarde en jeu, Kaleido relit le fichier et met à jour l'équipe, les boîtes et la progression, sans que tu aies à cliquer. Il ne modifie jamais ta sauvegarde.",
  },
  saveTiming: {
    title: "Quand la sauvegarde est lue",
    text: "Le journal et le suivi Nuzlocke se basent sur le fichier de sauvegarde : ils se mettent à jour quand tu sauvegardes en jeu (menu Sauvegarder). Entre deux sauvegardes, la lecture en direct montre déjà PV, rencontres et K.O. melonDS et Azahar écrivent le fichier tout de suite ; DeSmuME parfois seulement à la fermeture. L'heure affichée est celle de la dernière écriture du fichier.",
  },
  readOnly: {
    title: "Lecture seule",
    text: "Pendant la partie, le compagnon ne fait que lire. « Modifier » ouvre la sauvegarde dans l'éditeur, seulement émulateur fermé : écrire pendant qu'il tourne ferait perdre l'une des deux versions. L'éditeur garde une copie de secours avant d'écrire.",
  },
  hp: {
    title: "PV restants",
    text: "Points de vie en direct quand la mémoire de l'émulateur est lue, sinon au moment de la sauvegarde. À 0, le Pokémon est K.O. : il faut le soigner (Centre Pokémon, Rappel) avant qu'il puisse combattre.",
  },
  status: {
    title: "Problème de statut",
    text: "Effet qui reste après le combat : Sommeil (ne peut pas attaquer), Poison et Poison grave (perd des PV en marchant), Brûlure (attaque physique réduite), Gel (ne peut pas attaquer), Paralysie (Vitesse réduite, peut rater son tour).",
  },
  onTop: {
    title: "Toujours au premier plan",
    text: "Le compagnon reste visible par-dessus l'émulateur. Désactive-le si tu joues en plein écran.",
  },
  compact: {
    title: "Mode barre",
    text: "Réduit le compagnon à une seule ligne : les six Pokémon et leurs PV. Clique à nouveau pour revenir à la vue complète.",
  },
  autoOpen: {
    title: "Ouvrir à chaque partie",
    text: "Le compagnon s'ouvre tout seul quand tu lances ce jeu depuis la bibliothèque. Réglage mémorisé jeu par jeu.",
  },
  track: {
    title: "Suivi Nuzlocke automatique",
    text: "Kaleido compare chaque sauvegarde à la précédente : un nouveau Pokémon compte comme la capture de son lieu de rencontre, un Pokémon K.O. déposé en boîte ou relâché est mort. Les règles (doublons, surnoms, niveau maximum…) se règlent dans la page Nuzlocke de l'éditeur. Seule une rencontre ratée (fuite) est à signaler à la main.",
  },
  journal: {
    title: "Journal de partie",
    text: "Tout ce que le compagnon a vu changer d'une sauvegarde à l'autre, avec le temps de jeu et le lieu. Il est gardé par Kaleido, même si la sauvegarde est supprimée.",
  },
  nextBattle: {
    title: "Prochain combat",
    text: "Le prochain champion d'arène (ou le Conseil 4) d'après tes badges, avec son équipe lue dans ta ROM, randomisation comprise. Pour chacun de ses Pokémon, Kaleido indique ton meilleur contre et le résultat probable du duel. « Préparer » ouvre la page Combat pour le détail des dégâts.",
  },
  badges: {
    title: "Badges et épreuves",
    text: "Badges d'arène obtenus (Gen 4 à 6), ou épreuves des îles terminées en Soleil / Lune et Ultra. Lus dans la sauvegarde.",
  },
  live: {
    title: "Lecture en direct",
    text: "« En direct » : Kaleido lit la mémoire de l'émulateur et voit PV, statuts, rencontres et K.O. à la seconde, sans attendre une sauvegarde. « En jeu » : l'émulateur tourne mais sa mémoire n'est pas lue (jeu non pris en charge, lecture désactivée, partie pas encore chargée) ; le compagnon suit alors la sauvegarde. « Hors ligne » : aucun émulateur lancé. Le journal et les morts Nuzlocke restent comptés à la sauvegarde.",
  },
  memory: {
    title: "Lire la mémoire de l'émulateur",
    text: "Kaleido lit, sans jamais rien écrire, la mémoire de melonDS, DeSmuME ou Azahar pour suivre la partie en direct. Un antivirus peut signaler ce type de lecture : coupe-la ici si besoin, le compagnon reviendra à la lecture de la sauvegarde.",
  },
  encounter: {
    title: "Rencontre",
    text: "Pokémon sauvage vu dès l'entrée en combat. Une capture est reconnue toute seule ; sinon, indique l'issue : Raté (le sauvage est K.O.) ou Fui. En Nuzlocke, Raté et Fui marquent la route comme ratée, à la place du marquage à la main.",
  },
  battle: {
    title: "Combat en direct",
    text: "Équipe adverse lue dans la mémoire du jeu, avec ses PV. Disponible sur les jeux DS ; les jeux 3DS ne montrent que ton équipe.",
  },
  unverified: {
    title: "Lecture non vérifiée",
    text: "La lecture en direct de ce jeu n'a pas encore été vérifiée dans un émulateur. Si l'équipe affichée semble fausse, coupe la lecture de la mémoire : le compagnon reviendra à la sauvegarde.",
  },
};
