# Modèles des cartouches (mode Cartouche du lanceur)

Modèles 3D de vraies cartes et cartouches, téléchargés sur Sketchfab (format glTF), licence
CC-BY 4.0 : l'auteur doit être crédité. Le fichier `LICENSE.txt` de chaque dossier de
`app/public/models/` reprend le crédit d'origine.

| Support | Modèle | Auteur |
| --- | --- | --- |
| DS | [DS game card](https://sketchfab.com/3d-models/ds-game-card-822e8ff1a12946048a8fa9dbadf3379d) | maney |
| 3DS | [3DS Game Cartridge](https://sketchfab.com/3d-models/3ds-game-cartridge-950a6bf7c4094a84af0d7409d0e76c16) | JodieWebster |
| GBA | [Pokemon Cartridge (Gameboy)](https://sketchfab.com/3d-models/pokemon-cartridge-gameboy-7d79300f91a441d0ba520fdbd268aa5f) | thegraphicsgeek |
| Switch | [Nintendo Switch game Cartridge v2](https://sketchfab.com/3d-models/nintendo-switch-game-cartridge-v2-2e681c294545411d98eb61a98f2dba3e) | maxns1980 |

La cartouche Game Boy reste procédurale (`app/src/launcher/scene/geometry.ts`, contours SVG).

## Préparer les fichiers

Extraire chaque archive dans `tools/cartridges/sources/<nom de l'archive>/` (non commité), puis :

```
powershell -File tools/cartridges/textures.ps1
node tools/cartridges/gltf.mjs
```

`textures.ps1` réduit les textures (JPEG 1024 px au plus), remplace les étiquettes d'origine
par du papier vierge (l'étiquette du jeu est posée par-dessus, voir `realModels.ts`) et efface
l'inscription « GAME BOY ADVANCE SP » de la cartouche GBA. `gltf.mjs` réécrit les glTF pour
ces textures et ne garde qu'une des cinq cartes de la planche Switch.
