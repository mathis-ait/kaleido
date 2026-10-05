import type { GlossaryEntry } from "../../glossary";

/** Bulles « i » de la page Combat : chaque mécanique du calcul expliquée simplement. */
export const BATTLE_TIPS: Record<string, GlossaryEntry> = {
  linkRom: {
    title: "Lier une ROM",
    text: "Kaleido lit les dresseurs dans la ROM de ta partie : équipes, niveaux, attaques, objets, et les fiches des espèces (types, statistiques, talents). Avec une ROM randomisée, ce sont donc les vraies équipes que tu vas affronter. La ROM d'origine marche aussi.",
  },
  importantTrainers: {
    title: "Dresseurs importants",
    text: "Champions d'Arène, rivaux, chefs et admins des teams, Conseil 4 et Maître, classés dans l'ordre de l'histoire (estimé d'après le niveau de leur équipe). Les rivaux apparaissent en plusieurs versions : une par starter choisi.",
  },
  trainerIvs: {
    title: "IV des dresseurs",
    text: "Chaque Pokémon de dresseur a un niveau de « difficulté » de 0 à 255 : ses six IV valent difficulté × 31 / 255 (0 pour les dresseurs ordinaires, 31 pour les plus forts). Ils n'ont aucun EV.",
  },
  trainerMoves: {
    title: "Attaques des dresseurs",
    text: "Quand le dresseur n'a pas d'attaques fixées, ses Pokémon connaissent, comme dans le jeu, les 4 dernières attaques apprises par niveau jusqu'à leur niveau (lues dans la ROM, donc randomisées comprises).",
  },
  trainerNature: {
    title: "Nature des Pokémon adverses",
    text: "En Platine, la nature vient d'un « PID » que le jeu calcule à partir du numéro du dresseur, de l'espèce, du niveau et de la classe : Kaleido refait le même calcul. En Noir/Blanc et en 3DS, ce calcul n'est pas reproduit : une nature neutre est supposée.",
  },
  matrix: {
    title: "Matrice des duels",
    text: "Chaque case oppose un de tes Pokémon (ligne) à un Pokémon adverse (colonne) : ta meilleure attaque sur lui, sa meilleure attaque sur toi, et qui agit en premier. Clique sur une case pour le détail de toutes les attaques.",
  },
  verdict: {
    title: "Couleur des cases",
    text: "Vert : tu le mets K.O. avant qu'il ne te mette K.O., même avec de mauvais jets. Orange : ça dépend des jets aléatoires (ou d'une égalité de Vitesse). Rouge : il gagne même si tu as de la chance. Gris : personne ne peut blesser l'autre. Les coups critiques, la précision, les changements de stats et les soins ne sont pas comptés.",
  },
  stab: {
    title: "STAB (bonus de type)",
    text: "« Same Type Attack Bonus » : une attaque du même type que le Pokémon qui l'utilise fait ×1,5 de dégâts (×2 avec le talent Adaptabilité).",
  },
  effectiveness: {
    title: "Coefficient de type",
    text: "Multiplicateur selon le type de l'attaque et les types de la cible : ×2 par type faible (super efficace), ×0,5 par type résistant (pas très efficace), ×0 en cas d'immunité. Il se cumule : ×4, ×0,25… Depuis X/Y, le type Fée existe et l'Acier ne résiste plus aux attaques Spectre et Ténèbres.",
  },
  crit: {
    title: "Coup critique",
    text: "Un coup sur 16 environ fait ×2 de dégâts jusqu'en Noir/Blanc, ×1,5 depuis X/Y. Il ignore les baisses de ta statistique d'attaque et les hausses de la défense adverse. Il est affiché à part et n'est jamais compté dans les K.O.",
  },
  random: {
    title: "Aléatoire 85 à 100 %",
    text: "Chaque attaque multiplie ses dégâts par un nombre tiré au hasard parmi 16 valeurs, de 85 % à 100 %. D'où une fourchette « minimum – maximum » au lieu d'une seule valeur.",
  },
  category: {
    title: "Catégorie physique / spéciale",
    text: "Une attaque physique utilise l'Attaque du lanceur et la Défense de la cible ; une attaque spéciale utilise l'Attaque Spéciale et la Défense Spéciale. Depuis Diamant/Perle, la catégorie dépend de l'attaque elle-même (et non plus de son type).",
  },
  priority: {
    title: "Priorité",
    text: "Les attaques prioritaires (Vive-Attaque, Pisto-Poing : +1…) passent avant les autres quelle que soit la Vitesse. À priorité égale, le plus rapide agit en premier ; en cas d'égalité, c'est au hasard.",
  },
  ko: {
    title: "K.O. en N coups",
    text: "Nombre d'utilisations de l'attaque pour mettre la cible K.O. depuis ses PV actuels. « K.O. assuré en 2 coups » (2HKO) : même avec les plus mauvais jets. « 40 % de K.O. en 2 coups » : il faut de bons jets, sinon il en faut un de plus. Sans critique ni soin (Restes, baies).",
  },
  speed: {
    title: "Vitesse",
    text: "Vitesse finale, avec les niveaux de statistiques, le Mouchoir Choix (×1,5), la paralysie (×0,25 jusqu'en X/Y, ×0,5 ensuite), Glissade sous la pluie, Chlorophylle au soleil, etc.",
  },
  intimidate: {
    title: "Intimidation",
    text: "Le talent Intimidation baisse l'Attaque adverse d'un niveau à l'entrée en combat (sauf Corps Sain, Écran Fumée, Hyper Cutter…). Désactive-le si le Pokémon intimidant n'est pas encore sorti.",
  },
  weather: {
    title: "Météo",
    text: "Soleil : attaques Feu ×1,5, Eau ×0,5. Pluie : Eau ×1,5, Feu ×0,5. Tempête de sable : Défense Spéciale des Pokémon Roche ×1,5. Ciel Gris et Air Lock l'annulent. Grêle : rien sur les dégâts (sauf Ball'Météo).",
  },
  stages: {
    title: "Niveaux de statistiques",
    text: "Hausses et baisses en combat (Danse-Lames : +2 en Attaque, Rugissement : −1…), de −6 à +6 : +1 = ×1,5, +2 = ×2, −1 = ×2/3… Elles disparaissent quand le Pokémon est rappelé.",
  },
  status: {
    title: "Statut",
    text: "Brûlure : dégâts physiques ×0,5 (sauf avec Cran). Paralysie : Vitesse ×0,25 (×0,5 depuis Soleil/Lune). Cran, Pied Véloce, Écaille Spéciale et Façade en profitent.",
  },
  hp: {
    title: "PV restants",
    text: "Pourcentage de PV du Pokémon au début du duel. Il compte pour le K.O., pour Brasier / Torrent / Engrais / Essaim (×1,5 sous 1/3 des PV), pour Multiécaille (seulement PV pleins), Ceinture Force, Éruption, Gigotage…",
  },
  multihit: {
    title: "Attaques à plusieurs coups",
    text: "Double Pied frappe 2 fois, Triple Pied 3 fois ; Balle Graine, Furie, Stalactite… frappent de 2 à 5 fois : 3 coups sont comptés (5 avec le talent Multi-Coups). Les dégâts affichés additionnent tous les coups.",
  },
  fixed: {
    title: "Dégâts fixes",
    text: "Frappe Atlas et Ombre Nocturne infligent autant de PV que le niveau du lanceur, Draco-Rage 40, Sonic Boom 20, Croc Fatal la moitié des PV restants. Le type ne compte que pour les immunités.",
  },
  modifiers: {
    title: "Modificateurs",
    text: "Tout ce qui a changé les dégâts : STAB, objets (Bandeau Choix, Orbe Vie, Mouchoir Soie, Plaques, baies de résistance…), talents (Technicien, Isograisse, Filtre, Multiécaille, Peau Féérique…), météo, brûlure, critique. Les formules sont celles du calculateur de Pokémon Showdown.",
  },
};
