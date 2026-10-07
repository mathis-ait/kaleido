# Guide de style de l'interface

Kaleido doit ressembler à une console : sobre, lisible, la même d'une page à l'autre. La page **Sauvegarde** (Accueil, Boîtes, fiche Pokémon) est le modèle. Toute nouvelle page, y compris le compagnon de partie, part de ce guide.

## Règles

1. **Un seul jeu de composants.** Une page consomme les composants et classes ci-dessous, elle ne les réimplémente pas. Pas de `.chip`, `.round`, `.btn-line`, `.panel` ou `dl` redéfinis localement.
2. **Jetons, pas de valeurs en dur.** Espacements, rayons, tailles de texte et couleurs passent par les variables de `styles/main.css`. Une couleur hexadécimale dans une page est un bug, sauf dans une table de données (couleurs des types, familles de rencontre).
3. **Un « i » sur chaque terme technique**, alimenté par le glossaire central (`<Tip term="…">`). Aucun terme sans entrée, aucune bulle écrite en dur dans une page (`title=` / `text=` inline).
4. **Finition console.** Pas de barre d'accent colorée à gauche, pas d'emoji ni de symbole (★, ⚡) en guise de badge, pas de halo ni de pulsation animés, pas de dégradé décoratif. La sélection se montre par l'inversion texte / fond (pastille blanche en Lagon), comme les onglets.
5. **Clavier et manette partout.** Tout élément interactif est un vrai `<button>` (ou un champ), atteignable au Tab et à la croix de la manette, avec un focus visible (anneau `--focus`, global). Entrée et Espace activent le bouton qui a le focus ; les raccourcis de la barre du bas ne les volent pas.
6. **Trois états par page ou panneau** : vide (avec l'action pour en sortir), chargement, erreur en français lisible (avec « Réessayer » quand ça a du sens). Jamais d'erreur avalée en silence.
7. **Français simple, mêmes mots partout** : « N. 50 » pour le niveau, « Chromatique », « Talent », « Objet tenu », « Lieu de rencontre », « Dresseur d'origine ». Tutoiement.

## Jetons (`styles/main.css`)

| Famille | Variables | Usage |
| --- | --- | --- |
| Espacement | `--sp-1` 4 · `--sp-2` 8 · `--sp-3` 12 · `--sp-4` 16 · `--sp-5` 20 · `--sp-6` 24 | `gap`, `padding`, `margin` |
| Rayons | `--radius-panel` 18 · `--radius-card` 12 · `--radius-sm` 10 · `--radius-xs` 6 · `--radius-pill` | panneaux · cases et fiches · champs · petites étiquettes · pastilles et boutons |
| Texte | `--fs-xs` 11 · `--fs-sm` 12 · `--fs-md` 13 · `--fs-base` 14 · `--fs-lg` 16 · `--fs-xl` 20 | libellés en capitales · détails · corps des listes · corps · sous-titres · titres de panneau |
| Couleurs de thème | `--text`, `--text-dim`, `--bg`, `--panel`, `--panel-hover`, `--surface`, `--border`, `--accent*`, `--on-accent` | |
| Couleurs de sens | `--ok`, `--warn` / `--warn-bg`, `--danger`, `--shiny`, `--male`, `--female`, `--focus` | réussite, avertissement, erreur, chromatique, sexe, focus |

Le thème Pixel remet tous les rayons à 2–4 px : un rayon écrit en dur casse ce thème.

## Classes partagées (`save/form.css`, importé globalement)

| Classe | Rôle |
| --- | --- |
| `.sv-panel` | panneau en verre (rayon `--radius-panel`) |
| `.sv-btn`, `.sv-btn.solid`, `.sv-btn.danger` | bouton pastille ; `solid` pour l'action principale |
| `.sv-round` (`.sq` carré) | bouton d'icône rond : flèches de boîte, fermer |
| `.sv-chip` + `.on`, `.ok`, `.warn`, `.danger`, `.dim`, `.shiny` | étiquette ; `button.sv-chip` = filtre activable |
| `.sv-label` | libellé de section en capitales |
| `.sv-input`, `.sv-select` | champs |
| `.sv-dl` | fiche « libellé · valeur » (`dt` avec `<Tip>`, `dd` à droite) |
| `.sv-moves` | grille de 4 attaques |
| `.sv-section-title`, `.sv-help`, `.sv-row`, `.sv-grid`, `.sv-field` | mise en page des formulaires |
| `.sv-spin` | icône qui tourne (chargement) |

## Composants (`components/`)

| Composant | Rôle |
| --- | --- |
| `Tip` | bulle « i » ; toujours `term="…"` (glossaire central) |
| `Segmented` | choix exclusif (onglets internes, filtres à 2–4 valeurs), flèches au clavier |
| `Toggle` | interrupteur avec `term` facultatif |
| `SearchField` | recherche avec loupe, Échap vide le champ |
| `EmptyState` | état vide / chargement (`loading`) centré, avec slot `actions` |
| `Banner` | erreur ou avertissement, `retry` et `dismiss` |
| `Dialog` | fenêtre modale : Échap, clic à côté, focus gardé dans la fenêtre puis rendu |
| `Sprite`, `TypeBadge`, `Icon`, `Combo` | sprite du style choisi, type, icône, liste déroulante filtrable (choix d'espèce, d'attaque, d'objet) |

## Glossaire (`glossary.ts`)

Un seul point d'entrée : `GLOSSARY`. Les termes généraux (nature, talent, IV, lieu de rencontre…) n'ont pas de préfixe. Les termes propres à un module vivent dans son dossier et sont fusionnés avec un préfixe :

| Module | Fichier | Préfixe |
| --- | --- | --- |
| Banque | `save/bank/terms.ts` | `bank.` |
| Combat | `save/battle/glossary.ts` | `battle.` |
| Cadeaux | `save/gifts/terms.ts` | `gifts.` |
| Nuzlocke | `save/nuzlocke/terms.ts` | `nuzlocke.` |
| Jouer | `play/glossary.ts` | `play.` |
| Showdown | `save/showdown/glossary.ts` | aucun |

Avant d'ajouter un terme de module, vérifier qu'il n'existe pas déjà en terme général : un même mot doit avoir une seule explication.

## Manette

`gamepadNav.ts` donne la manette à toutes les pages : croix et stick pour aller à l'élément voisin dans cette direction, A pour activer, B pour Échap, LB / RB pour Q / E. Le lanceur garde sa propre gestion (`useGamepad`, qui met la navigation globale en retrait). Une page n'a rien à faire de spécial, à condition d'utiliser de vrais boutons.

## Vérifier

```
node scripts/pageshots.mjs <dossier> <sauvegarde> [préfixe] [pages] [thèmes] [largeurs]
```

capture chaque page dans les thèmes Lagon, Réseau, Pixel et Prisme Nuit, à 1280 et 1920 px (app lancée avec `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`).
