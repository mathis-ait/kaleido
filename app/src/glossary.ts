/**
 * Définitions courtes des termes techniques, affichées dans les bulles « i » (composant Tip).
 * Écrites pour quelqu'un qui découvre l'édition de sauvegardes.
 */
export interface GlossaryEntry {
  title: string;
  text: string;
}

export const GLOSSARY: Record<string, GlossaryEntry> = {
  // --- Pokémon : identité
  species: {
    title: "Espèce",
    text: "Le Pokémon lui-même (Carchacrok, Pikachu…). Changer d'espèce garde le niveau, les IV et les attaques ; pense à vérifier le talent et les attaques ensuite.",
  },
  form: {
    title: "Forme",
    text: "Variante d'une même espèce : Motisma Lavage, Prismillon Archipel, formes d'Alola… Une forme inexistante dans le jeu peut faire planter l'écran du Pokémon.",
  },
  nickname: {
    title: "Surnom",
    text: "Nom affiché en jeu. Sans surnom, le jeu affiche le nom de l'espèce dans la langue du Pokémon. Longueur maximale : 10 caractères sur DS, 12 sur 3DS.",
  },
  level: {
    title: "Niveau et expérience",
    text: "Le jeu ne stocke que les points d'expérience : le niveau en découle, selon la courbe de croissance de l'espèce (rapide, moyenne, lente…). Changer le niveau réécrit l'expérience.",
  },
  nature: {
    title: "Nature",
    text: "Augmente une statistique de 10 % et en baisse une autre de 10 % (5 natures sont neutres). En Gen 4, la nature est calculée à partir du PID : Kaleido recalcule le PID en gardant le sexe, le talent et le chromatique.",
  },
  ability: {
    title: "Talent",
    text: "Capacité passive (Voile Sable, Intimidation…). Chaque espèce a 1 ou 2 talents normaux et parfois un talent caché. L'« emplacement » (1, 2 ou caché) doit correspondre au talent choisi.",
  },
  hiddenAbility: {
    title: "Talent caché",
    text: "Troisième talent, plus rare : Dream World et Forêt Cachée (Gen 5), Safari des Amis (X/Y), Pokémon cachés (ROSA), appels à l'aide (Soleil/Lune)… Il n'existe pas en Gen 4.",
  },
  gender: {
    title: "Sexe",
    text: "Certaines espèces sont uniquement mâles, femelles ou asexuées. Jusqu'à la Gen 5, le sexe dépend aussi du PID : le changer seul peut rendre le Pokémon incohérent.",
  },
  shiny: {
    title: "Chromatique (shiny)",
    text: "Couleurs alternatives, environ 1 chance sur 8 192 (1/4 096 depuis X/Y). Il dépend du PID et des ID du dresseur d'origine. Clic : chromatique. Maj + clic : « carré ». Alt + clic : garde le PID et change l'ID secret, utile pour les Pokémon dont le PID est lié aux IV (Gen 3 à 5).",
  },
  heldItem: {
    title: "Objet tenu",
    text: "Objet porté par le Pokémon (Restes, Orbe Vie…). Un objet qui n'existe pas encore dans le jeu de la sauvegarde peut faire planter le jeu.",
  },
  friendship: {
    title: "Bonheur (amitié)",
    text: "De 0 à 255. Certaines espèces évoluent au-delà de 220 (Leveinard → Leuphorie, Évoli → Mentali / Noctali…). Retour et Frustration en dépendent aussi.",
  },
  language: {
    title: "Langue",
    text: "Langue du jeu où le Pokémon a été obtenu. Elle décide du nom affiché quand il n'a pas de surnom, et compte pour les échanges (Pokémon étrangers, méthode Masuda).",
  },
  egg: {
    title: "Œuf",
    text: "Le Pokémon est encore dans son œuf. Le bonheur sert alors de compteur de cycles avant éclosion.",
  },

  // --- Statistiques
  stats: {
    title: "Statistiques",
    text: "Valeurs finales (PV, Attaque…) calculées à partir des statistiques de base de l'espèce, du niveau, des IV, des EV et de la nature.",
  },
  iv: {
    title: "IV (valeurs individuelles)",
    text: "Le « patrimoine génétique » du Pokémon : de 0 à 31 par statistique, tiré au sort à la rencontre. 31 partout = parfait. Ils ne changent pas en jeu (sauf Entraînement Ultime en Gen 7). Jusqu'à la Gen 5, certains Pokémon sauvages ont des IV liés à leur PID : les modifier peut les rendre illégaux.",
  },
  ev: {
    title: "EV (points d'effort)",
    text: "Gagnés en battant des Pokémon ou avec des vitamines. 252 au maximum par statistique (255 avant la Gen 6, plafonné ici à 252) et 510 au total. Au niveau 100, 4 EV = 1 point de statistique.",
  },
  baseStats: {
    title: "Statistiques de base",
    text: "Valeurs propres à l'espèce, identiques pour tous les Carchacrok. Le graphique montre leur répartition.",
  },
  hiddenPower: {
    title: "Puissance Cachée",
    text: "Attaque dont le type dépend des IV (jusqu'à la Gen 7).",
  },

  // --- Attaques
  moves: {
    title: "Attaques",
    text: "Jusqu'à 4 attaques. Une attaque que l'espèce ne peut pas apprendre rend le Pokémon illégal (refusé en ligne et par les vérificateurs).",
  },
  pp: {
    title: "PP (points de pouvoir)",
    text: "Nombre d'utilisations restantes de l'attaque. Le maximum dépend de l'attaque et des PP Plus.",
  },
  ppUps: {
    title: "PP Plus",
    text: "De 0 à 3 : chaque PP Plus ajoute 20 % des PP de base de l'attaque (3 = PP Max).",
  },

  // --- Dresseur d'origine et identifiants
  ot: {
    title: "Dresseur d'origine (DO)",
    text: "Joueur qui a obtenu le Pokémon en premier. Un Pokémon échangé garde son DO : il obéit moins bien au-delà d'un certain niveau si les badges manquent.",
  },
  tid: {
    title: "ID dresseur",
    text: "Numéro public du dresseur (5 chiffres jusqu'à la Gen 6, 6 chiffres affichés depuis Soleil/Lune). Avec l'ID secret, il identifie le dresseur d'origine.",
  },
  sid: {
    title: "ID secret (SID)",
    text: "Second numéro du dresseur, jamais affiché en jeu. Il entre dans le calcul du chromatique.",
  },
  pid: {
    title: "PID (valeur personnelle)",
    text: "Nombre caché sur 32 bits tiré à la rencontre. Jusqu'à la Gen 5, il fixe la nature (Gen 4), le sexe, l'emplacement du talent et le chromatique. Le modifier au hasard rend souvent le Pokémon illégal : préfère les boutons de nature, de talent et de chromatique.",
  },
  ec: {
    title: "Constante de chiffrement (EC)",
    text: "Depuis la Gen 6, nombre caché qui chiffre les données du Pokémon. Il décide aussi de détails comme l'évolution de Chenipotte ou les taches de Spinda.",
  },
  tsv: {
    title: "TSV / PSV",
    text: "Valeurs chromatiques. Le Pokémon est chromatique quand la PSV (tirée du PID) égale la TSV (tirée de l'ID et de l'ID secret du dresseur). Utile pour faire éclore des œufs chromatiques.",
  },

  // --- Rencontre
  metLocation: {
    title: "Lieu de rencontre",
    text: "Où le Pokémon a été capturé, reçu ou est éclos. Les vérificateurs de légalité comparent ce lieu aux rencontres possibles de l'espèce.",
  },
  metLevel: {
    title: "Niveau de rencontre",
    text: "Niveau au moment de la capture (0 pour un Pokémon éclos d'un œuf en Gen 4). Il ne peut pas dépasser le niveau actuel.",
  },
  metDate: {
    title: "Date de rencontre",
    text: "Jour de la capture ou de l'éclosion, d'après l'horloge de la console.",
  },
  eggLocation: {
    title: "Lieu de l'œuf",
    text: "Où l'œuf a été reçu (Pension, échange…). Vide pour un Pokémon capturé.",
  },
  version: {
    title: "Jeu d'origine",
    text: "Version où le Pokémon a été obtenu. Un Pokémon de Platine transféré en Noire garde « Platine ».",
  },
  ball: {
    title: "Poké Ball",
    text: "Ball de capture, visible à l'envoi au combat. Certaines Balls n'existent pas dans tous les jeux (Balls d'Apricorne avant HGSS, Rêve Ball avant la Gen 5…).",
  },
  fateful: {
    title: "Rencontre fatidique",
    text: "Drapeau des Pokémon d'événement (distributions, Mew de Faraway Island…). Le cocher ou le décocher sans raison rend le Pokémon illégal.",
  },
  markings: {
    title: "Marquages",
    text: "Symboles (●▲■♥★♦) que le joueur pose pour trier ses boîtes. Purement décoratif. En Gen 7, chaque symbole peut être bleu ou rouge.",
  },
  pokerus: {
    title: "Pokérus",
    text: "Virus bénéfique : il double les EV gagnés. La souche dit combien de jours il reste à l'infection ; une fois guéri (0 jour), l'effet reste pour toujours.",
  },

  // --- Sauvegarde
  checksum: {
    title: "Somme de contrôle",
    text: "Empreinte que le jeu vérifie au démarrage. Si elle ne correspond pas, la sauvegarde est considérée corrompue. Kaleido la recalcule à chaque enregistrement.",
  },
  memecrypto: {
    title: "Signature Soleil / Lune",
    text: "Soleil, Lune, Ultra-Soleil et Ultra-Lune signent leurs sauvegardes (« MemeCrypto »). Kaleido ne sait pas encore recalculer cette signature : passe la sauvegarde modifiée dans PKHeX avant de la charger sur console.",
  },
  legality: {
    title: "Vérifications",
    text: "Kaleido contrôle la cohérence des données (sommes de contrôle, valeurs hors limites, talent, objets…). Ce n'est pas encore une analyse de légalité complète comme celle de PKHeX, qui compare chaque Pokémon à toutes les rencontres possibles.",
  },
  backup: {
    title: "Copie de sécurité",
    text: "Avant le premier enregistrement, Kaleido copie la sauvegarde d'origine à côté (« .kaleido.bak »). En cas de souci, renomme-la pour revenir en arrière.",
  },
  undo: {
    title: "Annuler / Rétablir",
    text: "Chaque modification est mémorisée (64 étapes) : Ctrl+Z annule, Ctrl+Y rétablit. Rien n'est écrit sur le disque avant « Enregistrer ».",
  },
  dragModes: {
    title: "Glisser-déposer",
    text: "Glisser : déplace ou échange. Maj + glisser : copie (le Pokémon d'origine reste). Alt + glisser : déplace en écrasant la cible (son ancien occupant est supprimé).",
  },

  // --- Dresseur
  money: {
    title: "Argent",
    text: "Plafonné à 999 999 ₽ dans les jeux ; Kaleido accepte jusqu'à 9 999 999 mais le jeu peut le ramener au maximum.",
  },
  playTime: {
    title: "Temps de jeu",
    text: "Compteur affiché sur la carte de dresseur. Il s'arrête à 999:59 dans les jeux.",
  },
  boxName: {
    title: "Nom de boîte",
    text: "Nom affiché en haut de la boîte dans le PC. 8 caractères sur DS, 16 sur 3DS.",
  },

  // --- Sac
  pouch: {
    title: "Poches du sac",
    text: "Le sac est rangé en poches (Soins, Poké Balls, CT…). Chaque poche n'accepte que certains objets et a un nombre de places limité.",
  },
  keyItems: {
    title: "Objets rares",
    text: "Objets d'histoire (Vélo, Canne…). En ajouter trop tôt peut bloquer un scénario : à manier avec prudence.",
  },
  tms: {
    title: "CT et CS",
    text: "Capsules Techniques (attaques réutilisables depuis la Gen 5) et Capsules Secrètes (attaques de terrain : Surf, Vol…). Leur quantité est toujours 1 dans les jeux récents.",
  },
  newFlag: {
    title: "Nouveau",
    text: "Marque « nouveau » affichée dans le sac en jeu. Purement visuel.",
  },
  favorite: {
    title: "Favori",
    text: "Objet épinglé en tête de liste ou dans le menu Y (selon le jeu).",
  },

  // --- Pokédex
  seen: {
    title: "Vu",
    text: "Le Pokémon a été rencontré (en combat, en échange…). Son image et sa zone d'habitat apparaissent dans le Pokédex.",
  },
  caught: {
    title: "Capturé",
    text: "Le Pokémon a été obtenu au moins une fois : sa fiche complète est débloquée. « Capturé » implique « vu ».",
  },
};
