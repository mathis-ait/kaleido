# Kaleido

Randomizer, éditeur de ROM et éditeur de sauvegardes Pokémon. Tout est en français, avec une interface moderne.
Le moteur est écrit en Rust et l'interface utilise Tauri 2 et Vue 3.

Inspirations : [Universal Pokémon Randomizer ZX](https://github.com/Ajarmar/universal-pokemon-randomizer-zx),
[pk3DS](https://github.com/kwsch/pk3DS), [pkNX](https://github.com/kwsch/pkNX),
[TidalHeX](https://github.com/HydrosPlays/TidalHeX) / PKHeX.

> Kaleido ne fournit aucune ROM. Utilise uniquement des dumps de tes propres cartouches.

## Jeux visés

| Fonction | Jeux pris en charge | État |
|---|---|---|
| Détection (ROM, dossier extrait, sauvegarde) | DS Gen 4/5, 3DS Gen 6/7 | ✓ |
| Pokédex (éditeur de ROM, lecture) | Platine, Noire, Blanche, Rubis Oméga, Saphir Alpha (vérifiés) ; autres jeux DS/3DS « non vérifiés » | ✓ |
| Randomizer → ROM `.nds` | Platine, Noire, Blanche | ✓ starters, sauvages, dresseurs, types, stats, talents |
| Randomizer → mod LayeredFS (Luma3DS ou émulateur) | Rubis Oméga, Saphir Alpha | ✓ sauvages, dresseurs, types, stats, talents (starters inchangés) |
| Éditeur de sauvegardes (interface façon TidalHeX) | Gen 4 à 7 | ✓ boîtes, fiche complète, dresseur, sac, Pokédex, noms des boîtes, vérifications ; vérifié sur le code de PKHeX et sur une vraie sauvegarde Soleil/Lune (signature recalculée) ; Gen 4 à 6 sans vraie sauvegarde de test |
| Switch | Let's Go, Épée, Bouclier, Légendes Arceus | Plus tard |

Aucune ROM randomisée n'a encore été testée en jeu : la validation porte sur la relecture
complète des fichiers produits (voir plus bas).

## Architecture

```
crates/formats   Formats bas niveau : NitroFS (ROM DS), NARC, LZ10/LZ11/BLZ, conteneurs 3DS,
                 RomFS (image ou dossier extrait), GARC, sortie LayeredFS
crates/core      Base des jeux, détection, textes Gen 4 à 7, données Pokémon
crates/cli       Outil `kaleido` pour inspecter et vérifier une ROM
app/             Interface Vue 3 (Vite)
app/src-tauri    Application de bureau Tauri, qui expose le cœur Rust à l'interface
```

### Outil en ligne de commande

```bash
cargo run --release -p kaleido-cli -- check   "rom.nds"                       # reconstruction, NARC, overlays
cargo run --release -p kaleido-cli -- species "rom.nds"                       # Pokédex
cargo run --release -p kaleido-cli -- find    "rom.nds" a/0/0/2 Bulbizarre    # recherche dans les textes
cargo run --release -p kaleido-cli -- textcheck "rom.nds" msgdata/pl_msg.narc # réencodage de tous les textes

# 3DS : image .3ds/.cxi/.cia déchiffrée, ou dossier extrait (romfs/ + exheader.bin)
cargo run --release -p kaleido-cli -- info3ds    "rom.3ds"                    # title ID, contenu du RomFS
cargo run --release -p kaleido-cli -- ls3ds      "rom.3ds" a/1/9              # fichiers, GARC, entrées LZ11
cargo run --release -p kaleido-cli -- check3ds   "rom.3ds"                    # reconstruction de tous les GARC
cargo run --release -p kaleido-cli -- species3ds "rom.3ds"                    # Pokédex Gen 6/7
cargo run --release -p kaleido-cli -- find3ds    "rom.3ds" a/0/7/4 Bulbizarre # recherche dans les textes
```

Emplacements vérifiés dans Rubis Oméga / Saphir Alpha (GARC v4) :

| Données | Archive | Format |
|---|---|---|
| Textes du jeu (FR) | `a/0/7/4` (une archive par langue de `a/0/7/1` à `a/0/7/8`) | espèces n°98, talents n°37, capacités n°14, objets n°113, types n°18, dresseurs n°22, classes n°21 |
| Textes de l'histoire (FR) | `a/0/8/2` (`a/0/7/9` à `a/0/8/6`) | |
| Fiches « personal » | `a/1/9/5` | 0x50 octets par espèce/forme ; la dernière entrée est la table complète |
| Attaques par niveau | `a/1/9/1` | `u16 attaque, u16 niveau`…, fin `FFFF FFFF` |
| Évolutions | `a/1/9/2` | 8 × (`u16 méthode, u16 paramètre, u16 espèce`) |
| Attaques œuf | `a/1/9/0` | `u16 nombre`, puis `u16` attaques |
| Méga-évolutions | `a/1/9/3` | 3 × (`u16 forme, u16 méthode, u16 objet, u16 —`) |
| Rencontres sauvages | `a/0/1/3` | fichiers de zone `ZO` (LZ11), section 4 ; copie concaténée dans l'entrée 537 (`EN`) |
| Dresseurs | `a/0/3/6` (trdata), `a/0/3/8` (trpoke), `a/0/3/7` (classes) | voir `crates/core/src/ctr_rom.rs` |
| Starters | `DllField.cro` @ `0xF906C` (Rev 2 Europe) | table des dons, 0x24 octets par entrée |

### État de la validation sur de vraies ROMs

| Vérification | Platine (Gen 4) | Blanche FR (Gen 5) | Rubis Oméga (Gen 6) |
|---|---|---|---|
| ROM reconstruite à l'identique | ✓ | ✓ | — (sortie LayeredFS) |
| Archives NARC / GARC reconstruites à l'octet près | 215 / 215 | 237 / 237 | 298 / 298 |
| Overlays BLZ / entrées LZ11 décompressées | — (aucun compressé) | 230 / 230 | 21 907 LZ11, recompressées sans erreur |
| Fichiers de texte réécrits à l'octet près (FR) | 724 / 724 | 760 / 760 | 175 / 175 + 637 / 637 |
| Chaînes réencodées à l'identique (FR) | 46 053 / 46 053 | 56 703 / 56 703 | 33 909 + 11 538 |
| Pokédex (noms, types, statistiques, talents) | ✓ | ✓ | ✓ (721 espèces) |

Diamant/Perle, HGSS, Noire 2/Blanche 2, X/Y et la Gen 7 utilisent des emplacements de données
pas encore vérifiés sur une vraie ROM (ils sont marqués « Non vérifié » dans l'application).

## Développement

Prérequis : Rust (rustup), Node.js LTS, Visual Studio Build Tools (C++).

```bash
cd app
npm install
npm run tauri dev
```

Pour lancer les tests du moteur :

```bash
cargo test --workspace --exclude kaleido-app
```

## Feuille de route

0. **Socle** : bibliothèque, glisser-déposer, détection des fichiers, thèmes ✓
1. **Formats DS** : NitroFS, NARC, LZ, textes Gen 4/5, Pokédex dans l'éditeur ✓
   Formats 3DS : RomFS, GARC, textes Gen 6/7, Pokédex Gen 6/7 (vérifié sur ROSA) ✓
2. **Randomizer DS** : Platine, Noire, Blanche ✓ — à faire : HGSS, Noire 2 / Blanche 2 (ROMs nécessaires)
3. **Randomizer 3DS** : ROSA ✓ (LayeredFS) — à faire : starters (module `.cro` signé), XY, SL, USUL
4. **Éditeur de sauvegardes** Gen 4 à 7 ✓ — accueil à tuiles, boîtes (glisser, Maj = copier, Alt = écraser,
   annuler / rétablir), fiche Pokémon en 6 onglets, dresseur, sac, Pokédex, gestionnaire de sauvegardes,
   signature Soleil / Lune. À faire : rubans, souvenirs, Cadeaux mystère, base des rencontres et légalité
   complète, banque de Pokémon entre jeux (façon PKForge), Gen 1 à 3 et Switch
5. **Switch**, sur le modèle de pkNX

## Sprites

Les icônes viennent de [pokesprite](https://github.com/msikma/pokesprite) (MIT), téléchargées
la première fois qu'elles s'affichent puis gardées en cache dans le dossier de l'application.
Les noms français embarqués (`crates/core/data/noms-fr.json`) sont extraits d'une ROM avec
`kaleido export-names`. Pokémon © Nintendo, Game Freak, The Pokémon Company.

Les données de l'éditeur de sauvegardes (`crates/core/src/dex/` : noms français, fiches des espèces,
attaques apprises, lieux de rencontre Gen 4 à 7) viennent des ressources de
[PKHeX](https://github.com/kwsch/PKHeX) de kwsch (GPLv3), voir `crates/core/data/pkhex/README.md`.
La puissance, la précision et la catégorie des attaques viennent de
[PokeAPI](https://github.com/PokeAPI/pokeapi) (BSD-3-Clause).

## Licence

GPL-3.0-or-later, comme les projets dont Kaleido s'inspire.
