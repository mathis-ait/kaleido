import type { GlossaryEntry } from "../glossary";

/** Bulles « i » propres au bouton « Jouer » (émulateurs, mods, synchro). */
export const PLAY_TIPS: Record<string, GlossaryEntry> = {
  emulator: {
    title: "Émulateur",
    text: "Programme qui fait tourner un jeu DS ou 3DS sur ton ordinateur. Kaleido ne fournit ni émulateur ni jeu : installe par exemple melonDS ou DeSmuME (DS), Azahar (3DS), puis Kaleido les retrouve tout seul.",
  },
  layeredfs: {
    title: "LayeredFS (mod)",
    text: "Système de mods des émulateurs 3DS : les fichiers modifiés par Kaleido sont lus dans load/mods/<title ID>/romfs à la place de ceux du jeu, sans toucher au jeu d'origine. Pour rejouer au jeu normal, renomme ou supprime ce dossier.",
  },
  titleId: {
    title: "Title ID",
    text: "Identifiant unique d'un jeu 3DS (16 chiffres hexadécimaux). Rubis Oméga : 000400000011C400 · Saphir Alpha : 000400000011C500. Le dossier du mod porte ce nom.",
  },
  saveDir: {
    title: "Dossier de sauvegarde",
    text: "melonDS écrit <nom de la ROM>.sav à côté de la ROM (sauf si un dossier est réglé dans melonDS) ; DeSmuME écrit <nom de la ROM>.dsv dans son dossier « Battery ». Ne change ce réglage que si ton émulateur range ses sauvegardes ailleurs.",
  },
  userDir: {
    title: "Dossier utilisateur 3DS",
    text: "Dossier où Azahar (ou Citra, Lime3DS) range la carte SD émulée (sdmc) et les mods (load). Par défaut %APPDATA%\\Azahar, ou le dossier « user » à côté de l'exécutable en version portable.",
  },
  formats: {
    title: "Fichiers .sav et .dsv",
    text: ".sav : sauvegarde brute, telle qu'elle est sur la cartouche (melonDS). .dsv : le même contenu suivi d'un petit pied ajouté par DeSmuME. Kaleido convertit automatiquement de l'un à l'autre quand il copie une sauvegarde.",
  },
  portable: {
    title: "Dossier portable",
    text: "Si ton émulateur n'est pas installé mais simplement décompressé quelque part (Téléchargements, clé USB…), ajoute ce dossier : Kaleido y cherche les exécutables, jusqu'à 3 niveaux de sous-dossiers.",
  },
  liveSync: {
    title: "Synchro en direct",
    text: "Kaleido surveille le fichier de sauvegarde du jeu. Quand tu sauvegardes en jeu, l'éditeur se recharge tout seul s'il n'a pas de modifications en cours ; sinon il te demande quoi faire. Rien n'est jamais écrasé sans copie de sécurité.",
  },
  sendToGame: {
    title: "Envoyer au jeu",
    text: "Écrit la sauvegarde modifiée là où l'émulateur la lit, après une copie de sécurité de l'ancienne. Ferme le jeu avant (ou redémarre-le après) : un jeu ouvert garde la sauvegarde en mémoire et l'écraserait à sa prochaine sauvegarde.",
  },
  backup: {
    title: "Copies de sécurité",
    text: "Avant de remplacer un fichier, Kaleido en garde une copie à côté : <fichier>.kaleido-<date>.bak. Un ancien mod 3DS est déplacé dans load/kaleido-backups. Pour revenir en arrière, renomme la copie.",
  },
  optimalSettings: {
    title: "Réglages optimaux",
    text: "Kaleido choisit les réglages graphiques adaptés à ta carte graphique (résolution, filtres, cache des shaders…) et les écrit dans la configuration de l'émulateur. Ton ancienne configuration est gardée de côté : « Restaurer » la remet exactement.",
  },
  otherMods: {
    title: "Autres mods",
    text: "Mods déjà présents dans le dossier de l'émulateur, installés sans Kaleido. Les désactiver les range dans un dossier de Kaleido (rien n'est supprimé) ; les réactiver les remet en place.",
  },
};
