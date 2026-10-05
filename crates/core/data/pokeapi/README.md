# Données d'attaques PokeAPI

Extraits de [PokeAPI](https://github.com/PokeAPI/pokeapi) (BSD-3-Clause),
commit `bc92d3b6029ef1abe9e7ad424c400b338f3c11fe`, dossier `data/v2/csv/`.

- `moves.csv` : colonnes `id, power, accuracy, priority, damage_class_id` de `moves.csv`,
  attaques 1 à 728 (valeurs actuelles). Catégorie : 1 statut, 2 physique, 3 spéciale.
- `move_changelog.csv` : lignes de `move_changelog.csv` (puissance, précision, priorité) dont le
  groupe de versions est postérieur à Diamant/Perle. Chaque ligne donne la valeur en vigueur
  *avant* ce groupe de versions.

Les PP et les types viennent de PKHeX (`../pkhex/moves/`).
