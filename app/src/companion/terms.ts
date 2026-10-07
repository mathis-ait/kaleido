import type { GlossaryEntry } from "../glossary";

/** Bulles « i » du compagnon de partie (glossaire central, préfixe « companion. »). */
export const COMPANION_TERMS: Record<string, GlossaryEntry> = {
  companion: {
    title: "Compagnon de partie",
    text: "Petite fenêtre à garder à côté de l'émulateur. À chaque sauvegarde en jeu, Kaleido relit le fichier et met à jour l'équipe, les boîtes et la progression, sans que tu aies à cliquer. Il ne modifie jamais ta sauvegarde.",
  },
  saveTiming: {
    title: "Quand la sauvegarde est lue",
    text: "Le compagnon lit le fichier de sauvegarde, pas la mémoire du jeu : il se met à jour quand tu sauvegardes en jeu (menu Sauvegarder). melonDS et Azahar écrivent le fichier tout de suite ; DeSmuME parfois seulement à la fermeture. L'heure affichée est celle de la dernière écriture du fichier.",
  },
  readOnly: {
    title: "Lecture seule",
    text: "Pendant la partie, le compagnon ne fait que lire. Pour modifier un Pokémon, ferme le jeu puis ouvre la sauvegarde dans l'éditeur : écrire pendant que l'émulateur tourne ferait perdre l'une des deux versions.",
  },
  hp: {
    title: "PV restants",
    text: "Points de vie au moment de la sauvegarde. À 0, le Pokémon est K.O. : il faut le soigner (Centre Pokémon, Rappel) avant qu'il puisse combattre.",
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
  badges: {
    title: "Badges et épreuves",
    text: "Badges d'arène obtenus (Gen 4 à 6), ou épreuves des îles terminées en Soleil / Lune et Ultra. Lus dans la sauvegarde.",
  },
};
