# Runtime ctr-smooth (phase 1) — Rubis Oméga EUR v1.0

## Principe retenu

La logique (`update()`) n'est jamais modifiée. Le jeu dessine désormais à chaque VBlank :

| Image | update() | Dessin | Matrices utilisées |
| --- | --- | --- | --- |
| A (avec update, tick n) | oui | **ajouté** | mélange 50 % tick n-1 / tick n |
| B (sans update) | non | d'origine | tick n, et enregistrées pour l'image suivante |

Sur l'image A, le tick n vient d'être calculé : le mélange (n-1, n) est dessiné tout de suite.
Sur l'image B, le jeu dessine l'état n comme il l'a toujours fait. **Aucune latence n'est ajoutée**
(le PRD prévoyait une demi-image : l'ordre ci-dessus l'évite). Les matrices remplacées pendant le
dessin A sont restaurées avant la fin de l'image : la logique ne voit jamais une valeur mélangée.

## Points d'interception

| Adresse | Instruction d'origine | Rôle |
| --- | --- | --- |
| 0x0010E530 | `ldrb r0,[r4,#0xd]` → `bl hook_gate` | décision « dessiner ? » : `smooth_gate(mgr, r6)` |
| 0x0010E534 | `rsb r1,r6,#0` → `cmp r0,#0` | |
| 0x0010E538 | `tst r0,r1` → `nop` | `bne 0x10E554` (inchangé) saute le dessin si r0 ≠ 0 |
| 0x0010E61C | `nop` → `bl smooth_end` | fin du dessin : restauration |
| 0x00375EA8 | `push {r0-r11,lr}` → `b hook_camera` | copie de la vue avant la file de rendu ; caméra = `[sp+0xC]` (`nw::gfx::Camera`) |
| 0x0039B338 | `push {r3-r11,lr}` → `b hook_h3d` | rendu d'un maillage H3D ; `r0` = modèle |
| 0x002EC354 | `push {r4-r8,lr}` → `b hook_nwmesh` | `nw::gfx` MeshRenderer ; `r2` = modèle |
| 0x0036FB34 | `add r1,r4,#0x98` → `bl hook_pad` | **builds de test seulement** : entrées injectées |

Matrices lissées (3×4, clé = adresse) :

- `nw::gfx::Camera` : vue `+0x148`, inverse de la vue `+0x178` (la position caméra).
- Modèles H3D (personnages, Pokémon, décors animés) : matrices **monde** des os,
  `*(modèle+0x4C)[0 .. *(modèle+0x40)[`. La translation du modèle y est incluse, donc
  déplacement et animation squelettique sont lissés ensemble.
- Modèles `nw::gfx` : matrice monde `+0x8C`.

Non couverts en phase 1 (prévus en phase 2) : palette d'os des `nw::gfx::SkeletalModel`
(0x002EC45C), particules, os « billboard » recalculés au dessin (0x0031DCCC, cohérents car
recalculés à partir de la vue lissée), coupures de caméra par événement (seuil seulement).

## Mémoire

| Zone | Adresse | Contenu |
| --- | --- | --- |
| Code | 0x00579610–0x0057A000 (marge de fin de `.text`) | 1,4 Ko (release), 1,5 Ko (test) |
| État `State` | 0x006AE640 (marge de fin de `.bss`, hors fichier) | initialisé au premier appel (magic `SMTH`) |
| Table | 0x0A000000, 256 Ko, `svcControlMemory` | 2 048 entrées de 108 octets ; script et instantané (tests) au-delà de 0x36000 |

Le `.bss` suit `.data` sans alignement : 0x0062F9F4 + 0x7EC2C = **0x006AE620**. La marge
« fin de .data » supposée en phase 0 était du `.bss` du jeu (corrigé).

Azahar n'applique pas d'`exheader.bin` de remplacement (seulement `code.ips` / `code.bps`) : le
`.bss` ne peut pas être agrandi, d'où l'allocation au démarrage. Si elle échoue, `failed` ≠ 0 et
le jeu garde son comportement d'origine.

## Structure `State` (0x006AE640, mots de 32 bits)

| Off. | Champ | | Off. | Champ |
| --- | --- | --- | --- | --- |
| 0x00 | magic `SMTH` | | 0x2C | n_interp |
| 0x04 | **enabled** (0 = original, à chaud) | | 0x30 | n_swap |
| 0x08 | jump_t (f32, 48.0) | | 0x34 | n_jump |
| 0x0C | jump_r (f32, 0.7) | | 0x38 | n_miss |
| 0x10 | tab_addr | | 0x3C | n_rec |
| 0x14 | tab_size | | 0x40 | vpad (tests) |
| 0x18 | tab | | 0x44 | n_skip |
| 0x1C | failed | | 0x48.. | script, script_pos, snap_frame, snap_a/alen, snap_b/blen, snap_dst, snap_done (tests) |
| 0x20 | frame | | | |
| 0x24 | interp | | | |
| 0x28 | lastrec | | | |

`tools/smstat.py` lit et modifie cette structure via le stub GDB (`set enabled 0`).

## Seuils (coupures, apparitions)

Un mélange est refusé si un coefficient de translation varie de plus de `jump_t` (48 unités,
soit environ 2,5 cases) ou un coefficient de rotation/échelle de plus de `jump_r` (0,7) entre
deux ticks : téléportation, changement de carte, coupure de caméra. L'image reprend alors le tick
courant sans mélange. Une matrice sans enregistrement à l'image précédente (objet apparu) n'est
pas mélangée.

## Construire

```bash
python runtime/build.py                      # release : runtime/build/code.ips
python runtime/build.py --counters --test-input -o runtime/build/code-test.ips
python runtime/build.py --merge kaleido-shiny.ips   # fusion avec un autre code.ips
```

`build.py` refuse un `code.bin` dont le SHA-256 diffère du profil et vérifie chaque instruction
remplacée.
