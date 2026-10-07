# Fichiers d'offsets de l'Universal Pokémon Randomizer ZX

Repris tels quels de [Universal Pokémon Randomizer ZX](https://github.com/Ajarmar/universal-pokemon-randomizer-zx)
(Dabomstew, Ajarmar et contributeurs, GPLv3), commit `7f00eb866ed35c8fe3963f078b6a2e0979dc2b8c`,
dossier `src/com/dabomstew/pkrandom/config/`.

| Ici | Lu par |
|---|---|
| `gen3_offsets.ini` | `crates/core/src/gba_rom.rs` (`parse_offsets`), emplacements des tables Rubis, Saphir, Émeraude, Rouge Feu, Vert Feuille par code jeu et version |
| `gen1_offsets.ini`, `gen2_offsets.ini` | `crates/core/src/gb_rom.rs` (Rouge, Bleu, Jaune, Or, Argent, Cristal) |

Les signatures de code (`Gen3Constants.java` : ordre du Pokédex, rencontres sauvages,
cartes, correctifs d'obéissance et d'évolution) sont recopiées dans le code Rust qui
les utilise, avec un renvoi au fichier Java d'origine.

Aucun de ces emplacements n'a été vérifié sur une vraie ROM pendant le développement :
les tests construisent des ROM synthétiques.
