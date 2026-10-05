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
crates/formats   Formats bas niveau : en-tête DS, conteneurs 3DS (CCI, CIA, CXI)…
crates/core      Base des jeux, détection des ROMs et sauvegardes, modèle de données
app/             Interface Vue 3 (Vite)
app/src-tauri    Application de bureau Tauri, qui expose le cœur Rust à l'interface
```

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
1. **Formats** : NitroFS, NARC, LZ77, RomFS, GARC, textes français
2. **Randomizer DS** : Platine et HGSS, puis Noire/Blanche 1 et 2
3. **Randomizer 3DS** : XY, ROSA, SL, USUL, avec sortie LayeredFS
4. **Éditeur de sauvegardes** Gen 4 à 7
5. **Switch**, sur le modèle de pkNX

## Licence

GPL-3.0-or-later, comme les projets dont Kaleido s'inspire.
