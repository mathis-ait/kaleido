# ctr-smooth — 60 fps natif 3DS (phase 0 : fondations)

Runtime et outillage du PRD « 60 FPS natif : 3DS (ROSA) puis toutes plateformes ». Principe : la
logique du jeu reste à 30 Hz (`update()` jamais modifié), `draw()` tourne à chaque VBlank et, sur
l'image sans tick logique, rend avec des matrices interpolées entre les deux derniers ticks.

## État de la phase 0 (8 octobre 2026)

Porte de phase : *liste des fonctions et structures, prototype `draw()` à chaque image qui compile et
tourne* — **atteinte** sur Rubis Oméga EUR v1.0 (cartouche Rev 2) dans Azahar 2126.1.2.

| Jalon | État |
| --- | --- |
| Outillage Ghidra (headless, scripts, RTTI → symboles) | fait, `re/` |
| Reproduction du patch de Zetta_D | fait : boucle `runEachFrame` retrouvée (0x0010E354, +8 par rapport à AS v1.4), `mode` = objet+0x0D = 0x08C650C1 |
| Prototype `draw()` à chaque image | fait et mesuré en jeu : update 29,6/s (inchangé), draw 59,2/s |
| Cartographie du pipeline de rendu | moteur identifié (NW4C `nw::gfx` + `gfl::grp::g3d` H3D), classes et vtables nommées, envoi exact des matrices restant à localiser (phase 1, étape 1) |
| Décision « où intercepter » | prise : côté consommateur dans `draw()`, par pipeline, clé = adresse du nœud + index d'os — voir `docs/rosa-render-map.md` § 3 |

Détails, mesures et pièges : [`docs/rosa-render-map.md`](docs/rosa-render-map.md).
Profil (brouillon) : [`profiles/000400000011C400-v1.0.fps60.yaml`](profiles/000400000011C400-v1.0.fps60.yaml).

## Arborescence

```
tools/azahar_gdb.py   client GDB RSP pour le stub d'Azahar : read / write / regs / watch / count
tools/azahar_win.ps1  capture de la fenêtre Azahar (PrintWindow) sans voler le focus
tools/mkpatch.py      fabrique / fusionne / affiche un code.ips
proto/instrument.py   compteurs de trace dans runEachFrame (+ variante draw à chaque image) → .ips
proto/*.ips           IPS générés (OR EUR v1.0) et copie du code.ips shiny de Kaleido
re/dis.py             désassemblage capstone avec résolution des littéraux
re/svc_scan.py        sites svc 0x28 (GetSystemTick) : repérage de la boucle principale
re/rtti.py            classes + vtables depuis le RTTI Itanium (fichier importable Ghidra)
re/pica_scan.py       mots de commande PICA (uniformes) et chaînes graphiques
re/ghidra/            CtrLayout.java (pré-script), Dump.java (post-script), run-headless.sh
re/out/               désassemblage / décompilation de la boucle, classes RTTI
profiles/             profil fps60.yaml par Title ID + hash de code.bin
docs/                 cartographie
```

## Mise en place (machine de développement)

Outils installés hors dépôt dans `C:\Users\Thisma\Documents\Switch\tools-re\` :
`jdk\` (Temurin 21.0.12), `ghidra\` (12.1.4), `gcc\` (xpack arm-none-eabi 15.2.1), projet Ghidra
`ghidra-proj\rosa` (code.bin analysé, symboles RTTI importés), `rosa-or\code.bin` + `exheader.bin`.
Python 3.12 (`%LOCALAPPDATA%\Programs\Python\Python312\python.exe`) avec `capstone`, `keystone-engine`,
`pyyaml`.

Extraction du code depuis une ROM déchiffrée (nouvelle commande CLI Kaleido) :

```bash
cargo run -p kaleido-cli -- code3ds "<rom>.3ds" <dossier>
```

Ghidra headless :

```bash
re/ghidra/run-headless.sh import                       # première analyse (≈ 4 min)
re/ghidra/run-headless.sh process -postScript Dump.java out.txt decomp 0x10e354
```

Mesure en jeu : activer le stub GDB d'Azahar (`[Debugging] use_gdbstub\default=false` et
`use_gdbstub=true`, Azahar fermé), déposer `proto/or-eur-v1.0-counters-draw60.ips` en
`%APPDATA%\Azahar\load\mods\000400000011C400\code.ips`, lancer le jeu puis :

```bash
python tools/azahar_gdb.py watch --secs 5 6AEFF0 3
```

Les trois mots lus sont : update, images sans update, draw (passages par seconde).

## Hors dépôt

Le fil GBAtemp de Zetta_D (texte) et le fichier de codes Reshiban sont conservés dans
`tools-re\refs\` ; aucune donnée du jeu n'est versionnée (seuls des IPS différentiels).
