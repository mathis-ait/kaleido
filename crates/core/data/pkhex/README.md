# Ressources PKHeX

Fichiers repris tels quels de [PKHeX](https://github.com/kwsch/PKHeX) (kwsch, GPLv3),
commit `2da87edbfd4e43a35b45500d15876165538bd769`, dossier `PKHeX.Core/Resources/`.
Ils sont embarqués par `crates/core/src/dex/`.

| Ici | Origine |
|---|---|
| `text/{Species,Moves,Abilities,Natures,Types,Games,Forms,Ribbons}.txt` | `text/other/fr/text_*_fr.txt` |
| `text/Items.txt`, `text/Mail4.txt` | `text/items/text_Items_fr.txt`, `text/items/gen4/text_Mail4_fr.txt` |
| `text/en/text_{Species,Moves,Abilities,Natures,Types,Forms}_en.txt` | `text/other/en/` (noms anglais, pour le format Showdown) |
| `text/en/text_Items_en.txt` | `text/items/text_Items_en.txt` |
| `text/met4_*.txt` | `text/locations/gen4/text_hgss_{00000,02000,03000}_fr.txt` |
| `text/met5_*.txt`, `met6_*`, `met7_*` | `text/locations/gen{5,6,7}/text_{bw2,xy,sm}_{00000,30000,40000,60000}_fr.txt` |
| `personal/personal_*` | `byte/personal/personal_{dp,pt,hgss,bw,b2w2,xy,ao,sm,uu}` |
| `levelup/lvlmove_*.pkl` | `byte/levelup/` (mêmes jeux) |
| `eggmove/eggmove_*.pkl` | `byte/eggmove/eggmove_{dppt,hgss,bw,xy,ao,sm,uu}.pkl` |
| `moves/pp_g{4,5,6,7}.txt`, `moves/type_g{5,9}.txt` | tableaux `PP` et `Type` de `PKHeX.Core/Moves/MoveInfo{4,5,6,7,9}.cs`, recopiés en texte |

Les formats binaires suivent `PersonalInfo4/5BW/5B2W2/6XY/6AO/7.cs`, `BinLinkerAccessor16.cs`,
`LearnsetReader.cs` et `MoveSource.cs`.

Puissance, précision, priorité et catégorie des attaques : voir `../pokeapi/README.md`.
