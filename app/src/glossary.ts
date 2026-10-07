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
  statShort: {
    title: "Abréviations des statistiques",
    text: "PV : points de vie. Att : Attaque. Déf : Défense. AtS : Attaque Spéciale. DéS : Défense Spéciale. Vit : Vitesse.",
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
    text: "Drapeau secret des Pokémon d'événement (cadeaux mystère, Mew de Faraway Island…). Il débloque certains comportements (Darkrai et l'Île Nouvellune, Shaymin et la Floraison) et sert de preuve de légitimité : le cocher ou le décocher sans raison rend le Pokémon illégal.",
  },
  markings: {
    title: "Marquages",
    text: "Symboles (●▲■♥★♦) que le joueur pose pour trier ses boîtes. Purement décoratif. En Gen 7, chaque symbole peut être bleu ou rouge.",
  },
  pokerus: {
    title: "Pokérus",
    text: "Virus bénéfique : il double les EV gagnés. La souche dit combien de jours il reste à l'infection ; une fois guéri (0 jour), l'effet reste pour toujours.",
  },
  pokerusStrain: {
    title: "Souche et jours du Pokérus",
    text: "La souche (1 à 15) fixe la durée de l'infection : souche % 4 + 1 jours (1 à 4). Les jours baissent à minuit ; à 0, le Pokémon est guéri et garde l'effet. Souche 0 = jamais infecté.",
  },
  learnable: {
    title: "Attaques apprenables",
    text: "Attaques que l'espèce peut apprendre dans ce jeu à son niveau : par niveau, CT/CS, donneurs de capacités et capacités Œuf. Les attaques d'événement ou héritées d'une autre génération n'y sont pas, même si elles sont légales.",
  },
  characteristic: {
    title: "Caractéristique",
    text: "Phrase du résumé (« Il adore manger. »…) qui trahit la meilleure IV et sa valeur modulo 5. Elle n'est pas stockée : le jeu la calcule depuis les IV (et le PID ou l'EC en cas d'égalité).",
  },
  hiddenPowerType: {
    title: "Choisir le type de Puissance Cachée",
    text: "Le type dépend de la parité des 6 IV. Kaleido change le moins d'IV possible, de 1 point chacune (31 devient 30) : l'effet sur les statistiques est minime.",
  },
  contest: {
    title: "Caractéristiques de concours",
    text: "Sang-froid, Beauté, Grâce, Intelligence et Robustesse (0 à 255) montent avec les Poffins (Gen 4) ou les Pokébloc (ROSA). Le Lustre limite le nombre de friandises qu'il peut encore manger.",
  },
  ribbons: {
    title: "Rubans",
    text: "Récompenses (concours, Tour de Combat, événements…). Purement décoratifs, mais un ruban impossible à obtenir rend le Pokémon illégal. Les rubans d'événement doivent correspondre à une vraie distribution.",
  },
  groundTile: {
    title: "Terrain de rencontre",
    text: "Type de sol où le Pokémon a été rencontré (herbe, grotte, eau…), qui fixait le décor du combat. Stocké en Gen 4 à 6 ; la légalité le vérifie pour les Pokémon de Gen 4.",
  },
  shinyLeaf: {
    title: "Feuilles brillantes",
    text: "HGSS : feuilles offertes par le Pokémon qui suit le joueur dans la Forêt de Jade (Pokéathlon). 5 feuilles permettent d'obtenir la couronne.",
  },
  walkingMood: {
    title: "Humeur en promenade",
    text: "HGSS : humeur du Pokémon qui suit le joueur, de -127 à 127. Elle change ses réactions quand on lui parle.",
  },
  nSparkle: {
    title: "Éclat de N",
    text: "N2B2 : drapeau des Pokémon de N rendus au joueur (ils scintillent au combat). À ne cocher que pour ces Pokémon précis.",
  },
  pokestarFame: {
    title: "Renommée Pokéstar",
    text: "N2B2 : popularité gagnée en tournant des films aux Studios Pokéstar.",
  },
  formArgument: {
    title: "Argument de forme",
    text: "Valeur liée à la forme : jours restants pour la coupe de Couafarel (5) ou Hoopa Déchaîné (3), avant de retrouver sa forme normale.",
  },
  hyperTraining: {
    title: "Hyper Training",
    text: "Soleil/Lune : M. Hyper entraîne une statistique contre une Capsule d'Argent. La statistique compte comme une IV à 31 en combat, sans changer l'IV stockée (donc ni la Puissance Cachée ni la caractéristique).",
  },
  handler: {
    title: "Soigneur",
    text: "Dernier dresseur, autre que le dresseur d'origine, à avoir reçu le Pokémon (échange, GTS…). Le jeu garde son nom, son affection et ses souvenirs. « Soigneur actuel » dit qui s'occupe du Pokémon aujourd'hui.",
  },
  country: {
    title: "Pays et région 3DS",
    text: "Pays, région et zone de la console du dresseur d'origine, réglés dans les paramètres de la 3DS. Les 5 lieux suivants retracent les pays des derniers échanges (le plus récent en premier).",
  },
  memories: {
    title: "Souvenirs",
    text: "X/Y à Ultra-Soleil/Ultra-Lune : un souvenir avec le dresseur d'origine et un avec le soigneur (« … se rappelle avoir combattu à vos côtés… »). Chaque souvenir a une intensité, un ressenti et parfois une variable : lieu, espèce, attaque ou objet.",
  },
  affection: {
    title: "Affection et Poké Récré",
    text: "Points d'affection de la Poké Récré (X/Y, ROSA) ou de la Pokémon-Récré (Soleil/Lune), de 0 à 255 : au-delà de 255 de bonheur, ils donnent des coups critiques, des esquives… Satiété et entrain servent aux friandises.",
  },
  superTraining: {
    title: "Super Training",
    text: "X/Y et ROSA : mini-jeu qui donne des EV. Chaque épreuve réussie donne une médaille. Les 8 dernières sont des épreuves distribuées.",
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
    title: "Vérifications (légalité)",
    text: "Comme PKHeX, Kaleido cherche une rencontre possible (hautes herbes, don, échange, œuf…) puis vérifie attaques, talent, Ball, PID, IV, dresseur… Verdict : Légal, Douteux (impossible à confirmer : événements, Pokémon des Gen 1 à 3) ou Illégal (le jeu ne peut pas le produire).",
  },
  legalize: {
    title: "Rendre légal",
    text: "Kaleido choisit la rencontre la plus proche (en gardant chromatique, nature, sexe, attaques légales…) et réécrit ce qu'il faut : lieu et niveau de rencontre, Ball, talent, PID et IV, attaques impossibles, dresseur d'origine. Tout se fait en une seule étape : Ctrl+Z annule.",
  },
  origin: {
    title: "Origine",
    text: "Jeu où le Pokémon a été obtenu (champ « version »). Un Pokémon venu d'un jeu plus ancien a été transféré (Pal Parc, Poké Transfert, Banque) : certaines données ont alors été réécrites.",
  },
  encounter: {
    title: "Rencontre",
    text: "Façon dont le Pokémon a été obtenu : sauvage (herbes, surf, pêche, Éclate-Roc…), fixe (légendaires), don d'un personnage, échange en jeu, œuf ou événement. Chaque rencontre impose un lieu, des niveaux et parfois la Ball, le talent ou le chromatique.",
  },
  encounterSlot: {
    title: "Emplacement de rencontre (slot)",
    text: "Une zone sauvage a une liste de Pokémon possibles, chacun avec une plage de niveaux et une probabilité : ce sont les « slots ». Le niveau de rencontre du Pokémon doit tomber dans la plage de son slot.",
  },
  pidiv: {
    title: "PID-IV (méthode 1)",
    text: "En Gen 3 et 4, le jeu tire le PID puis les IV à la suite avec le même générateur : les IV découlent donc du PID (« méthode 1 »). Modifier les IV ou la nature d'un Pokémon sauvage sans recalculer le PID le rend illégal. Exceptions : Poké Radar, Joli Sourire, Pokéwalker, œufs.",
  },
  method1: {
    title: "Méthode 1",
    text: "Algorithme de génération des Gen 3/4 : 4 tirages successifs donnent la moitié basse et haute du PID puis les deux moitiés des IV. Kaleido retrouve la graine pour vérifier le lien, et en génère une valide quand il crée un Pokémon.",
  },
  pidGen5: {
    title: "PID en Gen 5",
    text: "Noir/Blanc fixent le bit le plus haut du PID des Pokémon sauvages d'après l'ID du dresseur, et le talent d'après le bit 16 du PID. Un PID choisi au hasard est refusé une fois sur deux.",
  },
  transfer: {
    title: "Transfert",
    text: "Pal Parc (Gen 3 → 4), Poké Transfert (Gen 4 → 5), Poké Transporteur (Gen 5 → 6) et Banque (Gen 6 → 7, Console virtuelle). Le Poké Transfert remplace le lieu de rencontre ; vers la Gen 6, le PID devient la constante de chiffrement.",
  },
  shinyLock: {
    title: "Verrou chromatique",
    text: "Certains Pokémon (la plupart des légendaires depuis la Gen 5, dons, Trouées Cachées, beaucoup de cadeaux mystère…) ne peuvent jamais être chromatiques : le jeu recalcule le PID s'il tombe sur un chromatique. À l'inverse, certaines cartes cadeau l'imposent : PID chromatique fixe (identique pour tous) ou tiré au hasard puis rendu chromatique pour ton dresseur.",
  },
  flawlessIvs: {
    title: "IV parfaits garantis",
    text: "Depuis X/Y, les légendaires et les Pokémon du groupe Œuf « Inconnu » ont au moins 3 IV à 31 (2 au Safari des Amis). Moins que ça : illégal.",
  },
  relearn: {
    title: "Attaques à réapprendre",
    text: "Gen 6+ : liste cachée d'attaques retenues depuis la naissance (capacités Œuf) ou imposées par la rencontre. Un Pokémon sauvage ou transféré n'en a normalement aucune.",
  },
  eggOrigin: {
    title: "Pokémon né d'un œuf",
    text: "Un œuf garde le lieu où il a été reçu (Pension, échange…). À l'éclosion, il est « rencontré » au niveau 1 (0 en Gen 4) là où il est né. Jusqu'à la Gen 5, il est toujours dans une Poké Ball ; ensuite il hérite de la Ball d'un parent.",
  },
  learnset: {
    title: "Attaques possibles",
    text: "Une attaque est légale si l'espèce (ou une pré-évolution) l'apprend par niveau, par CT/CS, chez un donneur de capacités, comme capacité Œuf, ou si la rencontre la donne. Les attaques des jeux Gen 1 à 3 ne sont pas encore vérifiées.",
  },
  virtualConsole: {
    title: "Console virtuelle",
    text: "Rouge, Bleu, Jaune, Or, Argent et Cristal sur 3DS : leurs Pokémon arrivent dans Soleil/Lune par la Banque, avec leur talent caché, au moins 3 IV à 31, leur statut chromatique d'origine et un lieu de rencontre de transfert.",
  },
  encounterDb: {
    title: "Base des rencontres",
    text: "Toutes les rencontres connues de PKHeX pour les jeux Gen 4 à 7 : Pokémon sauvages, fixes, dons, œufs offerts, échanges, Rêve Radar, Pokéwalker, Monde des Rêves. « Créer ce Pokémon » en fabrique un, légal, dans le premier emplacement libre.",
  },
  onlyThisGame: {
    title: "Seulement ce jeu",
    text: "Coché : n'affiche que ce que le jeu de la sauvegarde ouverte peut obtenir (une carte cadeau réservée à X/Y n'apparaît pas pour Rubis Oméga). Pour les rencontres, décoché affiche aussi celles des jeux plus anciens dont les Pokémon peuvent être transférés dans cette sauvegarde (par exemple Platine ou Noir 2 pour Soleil/Lune).",
  },
  dreamWorld: {
    title: "Monde des Rêves",
    text: "Service en ligne de Noir/Blanc (Pokémon Global Link, fermé en 2014) : les Pokémon arrivaient au niveau 10 environ, avec leur talent caché, dans une Rêve Ball ou une Ball classique.",
  },
  safari: {
    title: "Safari",
    text: "Grand Marais (DPPt) et Parc Safari (HGSS) : capture avec des Safari Balls uniquement. Le Safari des Amis de X/Y donne 2 IV à 31 et parfois le talent caché.",
  },
  hordeSos: {
    title: "Hordes, PokéRadar Nav, appels à l'aide",
    text: "Rencontres spéciales qui peuvent donner le talent caché : hordes de 5 Pokémon (X/Y, ROSA), Pokémon cachés du PokéRadar Nav (ROSA, avec parfois une capacité Œuf), appels à l'aide (SOS) de Soleil/Lune.",
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

// --- Glossaires des modules, réunis ici pour que `<Tip term="…">` trouve tout au même endroit.
// Termes propres à un module : préfixe du module (« bank.format », « battle.stab »…).
import { COMPANION_TERMS } from "./companion/terms";
import { ADVENTURE_TERMS } from "./presets/terms";
import { PLAY_TIPS } from "./play/glossary";
import { BANK_TERMS } from "./save/bank/terms";
import { BATTLE_TIPS } from "./save/battle/glossary";
import { GIFT_TERMS } from "./save/gifts/terms";
import { NUZLOCKE_TERMS } from "./save/nuzlocke/terms";
import { SHOWDOWN_TERMS } from "./save/showdown/glossary";

function register(prefix: string, entries: Record<string, GlossaryEntry>) {
  for (const [key, entry] of Object.entries(entries)) GLOSSARY[prefix ? `${prefix}.${key}` : key] = entry;
}

register("", SHOWDOWN_TERMS);
register("play", PLAY_TIPS);
register("companion", COMPANION_TERMS);
register("adventure", ADVENTURE_TERMS);
register("bank", BANK_TERMS);
register("battle", BATTLE_TIPS);
register("gifts", GIFT_TERMS);
register("nuzlocke", NUZLOCKE_TERMS);
