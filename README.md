# Kaleido

Randomizer, éditeur de ROM et éditeur de sauvegardes Pokémon. Tout est en français, avec une interface moderne.
Le moteur est écrit en Rust et l'interface utilise Tauri 2 et Vue 3.

Inspirations : [Universal Pokémon Randomizer ZX](https://github.com/Ajarmar/universal-pokemon-randomizer-zx),
[pk3DS](https://github.com/kwsch/pk3DS), [pkNX](https://github.com/kwsch/pkNX),
[TidalHeX](https://github.com/HydrosPlays/TidalHeX) / PKHeX.

> Kaleido ne fournit aucune ROM. Utilise uniquement des dumps de tes propres cartouches.

## Jeux visés

| Plateforme | Jeux | État |
|---|---|---|
| Nintendo DS | Diamant, Perle, Platine, Or HeartGold, Argent SoulSilver, Noire, Blanche, Noire 2, Blanche 2 | Détection ✓ |
| Nintendo 3DS | X, Y, Rubis Oméga, Saphir Alpha, Soleil, Lune, Ultra-Soleil, Ultra-Lune | Détection ✓ |
| Switch | Let's Go, Épée, Bouclier, Légendes Arceus | Plus tard |

## Architecture

```
crates/formats   Formats bas niveau : NitroFS (ROM DS), NARC, LZ10/LZ11/BLZ, conteneurs 3DS
crates/core      Base des jeux, détection, textes Gen 4/5, données Pokémon
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
```

### État de la validation sur de vraies ROMs

| Vérification | Platine (Gen 4) | Blanche FR (Gen 5) |
|---|---|---|
| ROM reconstruite à l'identique | ✓ | ✓ |
| Archives NARC reconstruites à l'octet près | 215 / 215 | 237 / 237 |
| Overlays BLZ décompressés à la bonne taille | — (aucun compressé) | 230 / 230 |
| Fichiers de texte réécrits à l'octet près | 724 / 724 | 760 / 760 |
| Chaînes réencodées à l'identique | 46 053 / 46 053 | 56 703 / 56 703 |
| Pokédex (noms, types, statistiques, talents) | ✓ | ✓ |

Diamant/Perle, HGSS et Noire 2/Blanche 2 utilisent des emplacements de données pas encore
vérifiés sur une vraie ROM (ils sont marqués « Non vérifié » dans l'application).

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
   (RomFS / GARC pour la 3DS arrivent avec la phase 3)
2. **Randomizer DS** : Platine et HGSS, puis Noire/Blanche 1 et 2
3. **Randomizer 3DS** : XY, ROSA, SL, USUL, avec sortie LayeredFS
4. **Éditeur de sauvegardes** Gen 4 à 7
5. **Switch**, sur le modèle de pkNX

## Licence

GPL-3.0-or-later, comme les projets dont Kaleido s'inspire.
