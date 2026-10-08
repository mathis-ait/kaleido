# ctr-smooth — 60 fps natif 3DS (phases 0 et 1)

Runtime et outillage du PRD « 60 FPS natif : 3DS (ROSA) puis toutes plateformes ». Principe : la
logique du jeu reste à 30 Hz (`update()` jamais modifié), le jeu dessine à chaque VBlank, et l'image
ajoutée est rendue avec des matrices interpolées entre les deux derniers ticks.

Cible : Pokémon Rubis Oméga EUR, code cartouche v1.0 (Rev 2), Azahar 2126.1.2.

## État de la phase 1 — monde extérieur fluide (8 octobre 2026)

Porte du PRD : *Route 103 à 60 images distinctes, durées et RNG identiques à la référence* — **atteinte**.

| Critère | Mesure | Résultat |
| --- | --- | --- |
| 60 images distinctes | capture image par image en marchant sur la Route 103 (`tools/framecap.py`) | lissage actif : 29 transitions distinctes sur 29, écarts réguliers deux à deux ; désactivé : 14 sur 29 (une image sur deux en double) |
| Vitesse du jeu | compteurs dans `runEachFrame` | ticks logiques 28,7/s actif comme désactivé, dessins 57,6/s contre 28,7/s |
| Durées et RNG | 5 parties scriptées déterministes (`tools/replay_static.py`), lissage activé à l'apparition du joueur, instantané 800 images plus tard | 2 parties actives et 2 désactivées **identiques octet pour octet** : état complet du MT19937 (2 500 octets), matrice de vue, et trace image par image (rythme du RNG, trajectoire) |
| Aucun tirage pendant le dessin ajouté | point d'arrêt sur les 9 générateurs du jeu (`tools/rngcheck.py`) | 0 appel pendant un dessin interpolé, en combat et en marche |
| Coupures | seuils de saut (48 unités, 0,7) | environ 3 % des matrices refusées par image interpolée (os masqués par mise à l'échelle, apparitions), aucun glissement visible |
| Désactivation à chaud (F5) | `tools/smstat.py set enabled 0` | dessin d'origine (28,7/s), exactement les octets du jeu dans la boucle |

La cinquième partie scriptée (désactivée) a divergé des autres : c'est la variabilité propre au
démarrage d'Azahar, observée aussi entre deux parties désactivées, jamais liée au lissage.

Ce qui est lissé : caméra (vue et position), modèles H3D (personnages, Pokémon : déplacement **et**
animation squelettique, car leurs os sont en espace monde), modèles `nw::gfx`. Restent à 30 Hz en
phase 1 : palette d'os des `nw::gfx::SkeletalModel`, particules, effets de texture. Combats et
cinématiques fonctionnent déjà avec le runtime mais ne sont pas encore vérifiés (phase 2).

Fonctionnement, adresses, mémoire et seuils : [`docs/runtime.md`](docs/runtime.md).
Cartographie du moteur : [`docs/rosa-render-map.md`](docs/rosa-render-map.md).
Profil : [`profiles/000400000011C400-v1.0.fps60.yaml`](profiles/000400000011C400-v1.0.fps60.yaml).

## Essayer

`runtime/build/code.ips` (lissage seul) ou `runtime/build/code-with-kaleido-shiny.ips` (fusionné avec
le taux de shiny écrit par Kaleido) se dépose en
`%APPDATA%\Azahar\load\mods\000400000011C400\code.ips`. Rubis Oméga EUR **sans la mise à jour 1.4**
uniquement : sur une autre version, Azahar appliquerait le patch à des adresses fausses.

## Phase 0 (fondations) — rappel

Boucle `runEachFrame` en 0x0010E354 (adresses de Zetta_D sur AS v1.4 décalées de 8), octet `mode` en
gestionnaire+0x0D (0x08C650C1, cible des codes Reshiban), moteur NW4C `nw::gfx` + `gfl::grp::g3d`
H3D, 1 505 classes nommées par le RTTI.

## Arborescence

```
runtime/smooth.c      runtime C (interpolation, table, restauration, essais)
runtime/hooks.S       trampolines ARM des crochets
runtime/link.ld       placement dans les marges de code.bin
runtime/build.py      compilation + IPS vérifié (SHA-256, octets d'origine) ; --test-input, --script
tools/sandbox.sh      Azahar portable isolé (jamais l'Azahar de l'utilisateur), fenêtre discrète
tools/smstat.py       état du runtime via GDB : lecture, set enabled, watch, press (entrées injectées)
tools/gdbtrace.py     traces d'appels (entrée de fonction + adresse de retour), dumps mémoire
tools/framecap.py     capture image par image et comptage des images distinctes
tools/replay_static.py partie scriptée sans pause GDB ; snapdiff.py, verify_runs.py, tracediff.py
tools/rngcheck.py     tirages aléatoires pendant les dessins interpolés
tools/azahar_gdb.py   client GDB RSP ; mkpatch.py : IPS ; ini_set.py / sandbox_config.py : config
proto/replay/         script Route 103 et traces de la série de vérification finale
re/                   outillage Ghidra, désassemblage, RTTI, scans
```

## Mise en place (machine de développement)

Outils hors dépôt dans `C:\Users\Thisma\Documents\Switch\tools-re\` : `jdk\`, `ghidra\`, `gcc\`
(xpack arm-none-eabi 15.2.1), projet Ghidra `ghidra-proj\rosa`, `rosa-or\code.bin`, et
`azahar-sandbox\` (Azahar portable : config, sauvegarde et mods copiés, horloge fixe, rendu OpenGL,
stub GDB actif). Python 3.12 (`%LOCALAPPDATA%\Programs\Python\Python312\python.exe`) avec `capstone`,
`keystone-engine`, `numpy`, `pillow`.

```bash
python runtime/build.py
python runtime/build.py --counters --test-input -o runtime/build/code-test.ips
bash tools/sandbox.sh start runtime/build/code-test.ips && bash tools/sandbox.sh continue
python tools/smstat.py watch 3
```

Pièges : Azahar ne lit la manette qu'avec le focus (d'où l'injection), une fenêtre hors écran ne
reçoit rien, et une fenêtre Vulkan masquée ne se rafraîchit plus quand l'émulation est en pause
(d'où OpenGL dans le bac à sable). Détails dans `docs/rosa-render-map.md` § 5.
