/* ctr-smooth : structure d'état partagée (runtime ARM et code des essais Thumb). */
#ifndef CTR_SMOOTH_STATE_H
#define CTR_SMOOTH_STATE_H
typedef unsigned int u32;
typedef unsigned char u8;

typedef struct Entry Entry;

/* Configuration et statistiques : lisibles et modifiables à chaud (stub GDB,
 * Kaleido). L'ordre des champs est un format : voir docs/runtime.md. */
typedef struct {
    u32 magic;      /* 'SMTH' */
    u32 enabled;    /* 1 = interpolation active ; 0 = comportement d'origine */
    float jump_t;   /* plafond absolu de l'écart de translation entre deux ticks */
    float jump_r;   /* écart max d'un coefficient de rotation/échelle (objets) */
    u32 tab_addr;   /* adresse demandée pour la table */
    u32 tab_size;
    Entry *tab;     /* table allouée (0 = pas encore / échec) */
    u32 failed;     /* code d'erreur de l'allocation */
    u32 frame;      /* compteur d'appels de la décision de dessin */
    u32 interp;     /* le dessin en cours est interpolé */
    u32 lastrec;    /* dernière image dessinée sans interpolation */
    u32 n_interp;   /* dessins interpolés */
    u32 n_swap;     /* matrices mélangées (cumul) */
    u32 n_jump;     /* mélanges refusés par les seuils (cumul) */
    u32 n_miss;     /* table pleine (cumul) */
    u32 n_rec;      /* matrices enregistrées (cumul) */
    u32 vpad;       /* builds de test : boutons 3DS ajoutés à la manette (bits HID) */
    u32 n_skip;     /* dessins sautés par le jeu (surcharge, drapeau [gfx+0x1FE]) */
    /* Essais automatisés (builds --test-input) */
    const u32 *script; /* (image, boutons) ; 0xFFFF0000 sépare absolu / relatif à l'ancre */
    u32 script_pos;    /* (inutilisé, compatibilité de format) */
    u32 snap_frame;    /* image relative à l'ancre où copier, 0 = aucun */
    u32 snap_a, snap_alen, snap_b, snap_blen;
    u8 *snap_dst;
    u32 snap_done;
    u8 *cam;           /* dernière nw::gfx::Camera vue par hook_camera */
    u32 *trace;        /* essais : par image, (mot en snap_a, translation x de la vue) */
    u32 trace_n;
    u32 seen;          /* essais : 1 = squelette du joueur (50 os) déjà dessiné */
    u32 anchor;        /* essais : 1re image d'update après ce dessin (0 = pas encore) */
    /* Phase 2 */
    float jump_rel;    /* seuil de translation relatif : jump_rel * |t| + jump_min */
    float jump_min;
    float jump_rcam;   /* écart max d'un coefficient de rotation pour les caméras */
    u32 n_cut;         /* coupures de caméra détectées (cumul) */
    u32 upd;           /* la dernière image avait un update() */
    u32 c3d_u, c3d_n;  /* essais : passes de scène 3D (0x0038CEA4) sur images avec / sans update */
    u32 extra;         /* le dessin en cours est le dessin ajouté (image d'update, mode 30 Hz) */
    u32 n_fade;        /* dessins ajoutés pendant lesquels les transitions ont été figées (cumul) */
    /* Suite : caméras stéréoscopiques (yeux gauche / droit, intérieurs) */
    u8 *eye[4];        /* nw::gfx::Camera dont la vue est posée par SetViewMatrix (0x00392F90) */
    u32 eye_f[4];      /* valeur de frame lors de cette pose */
    u32 n_eye;         /* vues de caméras d'yeux lissées (cumul) */
    /* Suite : interrupteur en jeu */
    u32 pad;           /* boutons tenus au dernier tick (bits HID, gfl::ui::CTR_DeviceManager+0x78) */
    u32 combo_f;       /* frame + 1 au début de l'appui sur L + R + Select (0 = relâché) */
} State;

extern State S;

#define MAGIC 0x48544D53u /* 'SMTH' */

#endif
