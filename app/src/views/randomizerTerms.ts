import type { GlossaryEntry } from "../glossary";

/** Bulles « i » du randomizer détaillé (glossaire central, préfixe « randomizer. »). */
export const RANDOMIZER_TERMS: Record<string, GlossaryEntry> = {
  ctrOutput: {
    title: "Format de sortie",
    text: "LayeredFS : un dossier de mod léger, ta ROM reste intacte. Seuls les fichiers modifiés sont écrits ; Luma3DS ou l'émulateur les charge à la place des originaux. Fichier .3ds : une ROM complète à ouvrir directement dans Azahar/Citra, plus lourde (~2 Go). Elle est déchiffrée et ses signatures ne sont plus valides : parfait pour un émulateur. Sur une vraie console, il faut la convertir en CIA (par ex. « Build CIA from file » dans GodMode9) puis l'installer avec FBI sous Luma3DS — ou plus simplement utiliser la sortie LayeredFS.",
  },
  presets: {
    title: "Préréglages",
    text: "Remplace tous les réglages par une combinaison toute prête. Tu peux ensuite ajuster chaque option dans les onglets.",
  },
  randomTypes: {
    title: "Types aléatoires",
    text: "Une famille d'évolution garde les mêmes types",
  },
  randomAbilities: {
    title: "Talents aléatoires",
    text: "Chaque espèce reçoit des talents tirés au sort. Garde Mystik, Multitype, Illusion et Mode Transe ne sont jamais attribués.",
  },
  noLegendaries: {
    title: "Sans légendaires",
    text: "Aucun légendaire ni fabuleux n'est tiré au sort pour remplacer un autre Pokémon.",
  },
  easyEvolutions: {
    title: "Évolutions sans échange",
    text: "Les évolutions par échange se font au niveau 37, ou avec l'objet habituel (Peau Métal…)",
  },
  randomMovesets: {
    title: "Attaques apprises aléatoires",
    text: "Chaque Pokémon garde sa première attaque, les suivantes sont tirées au hasard",
  },
  kantoStarters: {
    title: "Pokémon de Kanto",
    text: "Dans X et Y, le Professeur Platane offre à Illumis un second starter au choix : Bulbizarre, Salamèche ou Carapuce (niveau 10). Kaleido les remplace sans reprendre les starters ci-dessus ; le niveau 10 est conservé.",
  },
  wildSimilarStrength: {
    title: "Puissance similaire",
    text: "Un Pokémon sauvage est remplacé par une espèce de force comparable (total des statistiques de base proche) : pas de Dracolosse sur la Route 1.",
  },
  trainersSimilarStrength: {
    title: "Puissance similaire",
    text: "Chaque Pokémon des dresseurs est remplacé par une espèce de force comparable, pour garder la difficulté d'origine.",
  },
  trainerEvolutions: {
    title: "Pokémon évolués selon leur niveau",
    text: "Un Machoc niveau 40 devient Mackogneur… (niveau 40 pour les évolutions sans niveau)",
  },
  trainerMaxIvs: {
    title: "IV au maximum",
    text: "Tous les Pokémon des dresseurs ont des IV parfaits",
  },
  randomTms: {
    title: "CT aléatoires",
    text: "Les CS ne changent jamais",
  },
  randomTutors: {
    title: "Maîtres des capacités aléatoires",
    text: "Platine, Noire 2 et Blanche 2",
  },
  keepFieldMoves: {
    title: "Garder les attaques de terrain",
    text: "Tunnel, Flash… restent à leur place",
  },
  noGameBreaking: {
    title: "Sans Sonicboom / Draco-Rage",
    text: "Ces attaques infligent des dégâts fixes (20 et 40 PV) : très fortes en début de partie, elles cassent l'équilibre.",
  },
  fullHmCompat: {
    title: "Toutes les CS pour tous",
    text: "Pratique pour ne jamais être bloqué",
  },
  followEvolutions: {
    title: "Les évolutions héritent",
    text: "Une évolution garde les compatibilités CT de sa forme précédente (plus logique : Dracaufeu sait tout ce que savait Salamèche).",
  },
  levelupSanity: {
    title: "Garder les CT des attaques apprises",
    text: "Si un Pokémon apprend une attaque par niveau, il reste compatible avec la CT de cette attaque.",
  },
  banBadFieldItems: {
    title: "Pas d'objets inutiles",
    text: "Lettres, Fertilisants, Baies sans effet…",
  },
  guaranteeEvolutionItems: {
    title: "Pierres d'évolution en vente",
    text: "Les Pierres Feu, Eau, Foudre… et autres objets d'évolution sont toujours achetables quelque part, pour ne pas bloquer une évolution.",
  },
  guaranteeXItems: {
    title: "Objets X en vente",
    text: "Les objets X (Attaque +, Défense +, Vitesse +, Précision +…) augmentent une statistique pendant un combat. Cette option les garde en vente dans les boutiques après randomisation, pratique contre les combats difficiles.",
  },
  banOpShopItems: {
    title: "Pas d'objets trop forts en boutique",
    text: "Super Bonbon, Pépites, Œuf Chance…",
  },
  noRareCandy: {
    title: "Sans Super Bonbon",
    text: "Le Super Bonbon (un niveau gratuit) n'apparaît ni au sol ni en boutique.",
  },
  noMasterBall: {
    title: "Sans Master Ball",
    text: "La Master Ball (capture garantie) n'apparaît pas dans les objets randomisés.",
  },
  statics: {
    title: "Pokémon fixes et dons",
    text: "Les Pokémon qu'on rencontre à un endroit précis (légendaires, Ronflex qui bloque la route…) et ceux qu'on reçoit en cadeau (fossiles, œufs, starters secondaires).",
  },
  staticLevels: {
    title: "Niveau des Pokémon fixes",
    text: "Augmente ou baisse le niveau de ces rencontres (les œufs ne changent pas). +20 % : un légendaire niveau 70 passe niveau 84.",
  },
  trades: {
    title: "Échanges en jeu",
    text: "Les Pokémon que des personnages proposent d'échanger contre l'un des tiens (par exemple Kéké le Chétiflor dans Platine).",
  },
  tradeRandomItems: {
    title: "Objets tenus aléatoires",
    text: "Le Pokémon reçu tient un objet tiré au sort.",
  },
  tradeRandomIvs: {
    title: "IV aléatoires",
    text: "Les IV (le « potentiel génétique ») du Pokémon reçu sont tirés au sort au lieu d'être fixés par le jeu.",
  },
  codeBin: {
    title: "code.bin",
    text: "Le programme du jeu 3DS. Kaleido y change le nombre de PID tirés à chaque Pokémon créé : le jeu garde le premier chromatique trouvé. En mod LayeredFS, c'est un petit fichier code.ips à côté du dossier romfs ; dans une ROM .3ds complète, le programme est réécrit directement.",
  },
  showStarters: {
    title: "Voir",
    text: "Les starters restent cachés pour garder la surprise",
  },
};
