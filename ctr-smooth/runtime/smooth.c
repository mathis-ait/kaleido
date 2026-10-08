/*
 * ctr-smooth — runtime d'interpolation de rendu (Pokémon Rubis Oméga EUR v1.0).
 *
 * Principe (PRD option B) : la logique tourne à 30 Hz sans aucune modification ;
 * le jeu dessine à chaque VBlank. Sur l'image où update() vient de produire le
 * tick n, les matrices consommées par le rendu sont remplacées le temps du dessin
 * par le mélange 50 % (tick n-1, tick n), puis restaurées. Sur l'image suivante
 * (sans update), le jeu dessine l'état n tel quel : c'est le dessin d'origine.
 * Aucune latence n'est ajoutée et l'état du jeu est intact entre deux dessins.
 *
 * Le runtime n'écrit que dans sa table (mémoire allouée au démarrage) et, le
 * temps d'un dessin, dans les matrices qu'il restaure avant la fin de l'image.
 *
 * Contraintes : ARMv6K (ARM11 MPCore, pas de Thumb-2), VFPv2, pas de libc ;
 * le code tient dans la marge de fin de .text (2,5 Ko), l'état dans la marge
 * de fin de .bss (page 0x006AE620-0x006AF000, hors fichier : initialisé au
 * premier appel), la table dans un bloc alloué par svcControlMemory.
 */
typedef unsigned int u32;
typedef unsigned char u8;

#define TAB_BITS 11u
#define TAB_N (1u << TAB_BITS)
#define TAB_PROBES 16u
#define STALE_FRAMES 120u

typedef struct {
    u32 key;        /* adresse de la matrice dans la mémoire du jeu (0 = libre) */
    u32 rec;        /* image où prev a été enregistrée */
    u32 swp;        /* image où la matrice a été remplacée par le mélange */
    float prev[12]; /* matrice 3x4 du tick précédent */
    float orig[12]; /* valeur d'origine, restaurée en fin d'image */
} Entry;

/* Configuration et statistiques : lisibles et modifiables à chaud (stub GDB,
 * Kaleido). L'ordre des champs est un format : voir docs/runtime.md. */
typedef struct {
    u32 magic;      /* 'SMTH' */
    u32 enabled;    /* 1 = interpolation active ; 0 = comportement d'origine */
    float jump_t;   /* écart de translation au-delà duquel on n'interpole pas */
    float jump_r;   /* écart d'un coefficient de rotation/échelle idem */
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
    /* Essais automatisés (builds --test-input) : entrées scriptées par numéro
     * d'image et instantané mémoire à une image donnée. */
    const u32 *script; /* paires (image, boutons) croissantes, fin = 0xFFFFFFFF */
    u32 script_pos;
    u32 snap_frame;    /* image (valeur de frame) où copier, 0 = aucun */
    u32 snap_a, snap_alen, snap_b, snap_blen;
    u8 *snap_dst;
    u32 snap_done;
    u8 *cam;           /* dernière nw::gfx::Camera vue par hook_camera */
    u32 *trace;        /* essais : par image, (mot en snap_a, translation x de la vue) */
    u32 trace_n;
    u32 seen;          /* essais : 1 = squelette du joueur (50 os) déjà dessiné */
    u32 anchor;        /* essais : 1re image d'update après ce dessin (0 = pas encore) */
} State;

__attribute__((section(".state"), used)) State S;

#define MAGIC 0x48544D53u /* 'SMTH' */

#ifdef TEST_SCRIPT
#include "build/test_script.h" /* généré par build.py --script */
#endif

static u32 svc_alloc(u32 addr, u32 size, u32 *out)
{
    register u32 r0 __asm__("r0") = 3u;  /* MEMOP_ALLOC */
    register u32 r1 __asm__("r1") = addr;
    register u32 r2 __asm__("r2") = 0u;
    register u32 r3 __asm__("r3") = size;
    register u32 r4 __asm__("r4") = 3u;  /* MEMPERM_READ | MEMPERM_WRITE */
    __asm__ volatile("svc 0x01" : "+r"(r0), "+r"(r1), "+r"(r2), "+r"(r3) : "r"(r4) : "memory", "r12", "lr");
    *out = r1;
    return r0;
}

static Entry *find(u32 key)
{
    Entry *t = S.tab, *cand = 0;
    u32 h = (key * 2654435761u) >> (32u - TAB_BITS);
    for (u32 i = 0; i < TAB_PROBES; i++) {
        Entry *e = &t[(h + i) & (TAB_N - 1u)];
        if (e->key == key)
            return e;
        if (!cand && (e->key == 0 || S.frame - e->rec > STALE_FRAMES))
            cand = e;
        if (e->key == 0)
            break;
    }
    if (cand) {
        cand->key = key;
        cand->rec = 0;
        cand->swp = 0;
    } else {
        S.n_miss++;
    }
    return cand;
}

static inline float fabs_(float x) { return x < 0.0f ? -x : x; }

/* Point d'interception commun : `m` est une matrice 3x4 que le rendu va lire. */
static void smooth(float *m)
{
    Entry *e = find((u32)m);
    if (!e)
        return;
    if (!S.interp) {
        for (u32 i = 0; i < 12; i++)
            e->prev[i] = m[i];
        e->rec = S.frame;
        S.n_rec++;
        return;
    }
    if (e->rec != S.lastrec || e->swp == S.frame)
        return;
    for (u32 i = 0; i < 12; i++) {
        float lim = (i & 3u) == 3u ? S.jump_t : S.jump_r;
        if (fabs_(m[i] - e->prev[i]) > lim) {
            S.n_jump++;
            return;
        }
    }
    for (u32 i = 0; i < 12; i++) {
        e->orig[i] = m[i];
        m[i] = (m[i] + e->prev[i]) * 0.5f;
    }
    e->swp = S.frame;
    S.n_swap++;
}

/* Remplace la décision « dessiner ? » de runEachFrame (0x0010E530).
 * Renvoie 1 pour sauter le dessin (comportement d'origine), 0 pour dessiner. */
u32 smooth_gate(const u8 *mgr, u32 did_update)
{
    u32 mode = mgr[0x0D];
    if (S.magic != MAGIC) {
        u32 *w = (u32 *)&S;
        for (u32 i = 0; i < sizeof(S) / 4u; i++)
            w[i] = 0;
        S.magic = MAGIC;
        S.enabled = 1u;
        S.jump_t = 48.0f;
        S.jump_r = 0.7f;
        S.tab_addr = 0x0A000000u;
        S.tab_size = 0x40000u;
#ifdef TEST_SCRIPT
        S.enabled = 0u; /* le mode testé s'applique à partir de l'ancre */
#endif
    }
    if (!S.tab && !S.failed) {
        u32 addr = 0, rc = svc_alloc(S.tab_addr, S.tab_size, &addr);
        if (rc == 0 && addr) {
            S.tab = (Entry *)addr;
#ifdef TEST_SCRIPT
            /* partie scriptée sans aucune pause GDB (déterminisme) */
            S.script = test_script;
            S.snap_frame = TEST_SNAP_REL;
            S.snap_a = TEST_SNAP_A;
            S.snap_alen = TEST_SNAP_ALEN;
            S.snap_b = 1u;
            S.snap_blen = 0x30u;
            S.snap_dst = (u8 *)addr + 0x38000u;
            S.trace = (u32 *)((u8 *)addr + 0x39000u);
            S.trace_n = TEST_SNAP_REL + 1u;
#endif
        } else {
            S.failed = rc ? rc : 1u;
        }
    }
    u32 on = S.enabled && S.tab;
    u32 skip = (mode && did_update) && !on;
    /* Le jeu saute lui-même le prochain dessin après une image trop lente
     * ([[mgr+0x1C]+0x1FE], testé juste après cette fonction) : pas d'interpolation. */
    const u8 *gfx = *(const u8 *const *)(mgr + 0x1C);
    if (!skip && gfx && gfx[0x1FE])
        S.n_skip++, skip = 2u;
    S.frame++;
#ifdef TEST_SCRIPT
    if (S.seen && !S.anchor && did_update) {
        S.anchor = S.frame;
        S.enabled = TEST_ENABLED;
        /* graine MT19937 fixe à l'ancre : la graine du jeu dépend de l'instant du
         * démarrage, ce qui rendrait les rencontres différentes d'une partie à l'autre */
        u32 *mt = (u32 *)TEST_SNAP_A;
        mt[1] = 0x5EED1234u;
        for (u32 i = 1; i < 624u; i++)
            mt[i + 1] = 1812433253u * (mt[i] ^ (mt[i] >> 30)) + i;
        mt[0] = 0;
        on = S.enabled && S.tab;
    }
    u32 rel = S.anchor ? S.frame - S.anchor : 0xFFFFFFFFu;
#else
    u32 rel = S.frame;
#endif
    if (S.snap_frame && rel == S.snap_frame && !S.snap_done && S.snap_dst) {
        u8 *d = S.snap_dst;
        for (u32 i = 0; i < S.snap_alen; i++)
            *d++ = ((const u8 *)S.snap_a)[i];
        /* snap_b == 1 : vue de la dernière caméra du terrain */
        const u8 *b = S.snap_b == 1u ? (S.cam ? S.cam + 0x148 : 0) : (const u8 *)S.snap_b;
        for (u32 i = 0; b && i < S.snap_blen; i++)
            *d++ = b[i];
        S.snap_done = 1u;
    }
    if (S.trace && rel < S.trace_n) {
        S.trace[2 * rel] = S.snap_a ? *(const u32 *)S.snap_a : 0;
        S.trace[2 * rel + 1] = S.cam ? *(const u32 *)(S.cam + 0x148 + 12) : 0;
    }
    S.interp = on && mode && did_update && !skip;
    if (S.interp)
        S.n_interp++;
    return skip == 1u;
}

/* Fin du dessin (0x0010E61C) : restaure les matrices remplacées. */
void smooth_end(void)
{
    if (!S.tab)
        return;
    if (S.interp) {
        Entry *e = S.tab;
        for (u32 i = 0; i < TAB_N; i++, e++) {
            if (e->swp == S.frame) {
                float *m = (float *)e->key;
                for (u32 k = 0; k < 12; k++)
                    m[k] = e->orig[k];
            }
        }
    } else {
        S.lastrec = S.frame;
    }
    S.interp = 0;
}

/* nw::gfx::Camera : vue (+0x148) et son inverse (+0x178). */
void smooth_camera(u8 *cam)
{
    if (!S.tab || !cam)
        return;
    S.cam = cam;
    smooth((float *)(cam + 0x148));
    smooth((float *)(cam + 0x178));
}

/* Modèle H3D (gfl::grp::g3d) : matrices monde des os, *(+0x4C)[0 .. *(+0x40)[. */
void smooth_h3d(u8 *model)
{
    if (!S.tab || !model)
        return;
    u32 n = *(u32 *)(model + 0x40);
    float *bones = *(float **)(model + 0x4C);
    if (!bones || n > 256u)
        return;
#ifdef TEST_SCRIPT
    if (n == 50u)
        S.seen = 1u;
#endif
    for (u32 i = 0; i < n; i++)
        smooth(bones + i * 12u);
}

/* Modèle nw::gfx : matrice monde (+0x8C). */
void smooth_nwmodel(u8 *model)
{
    if (!S.tab || !model)
        return;
    smooth((float *)(model + 0x8C));
}

/* Builds de test : boutons tenus = manette + S.vpad + script. Le script a deux
 * parties : images absolues (menus, jusqu'à l'ancre), séparateur 0xFFFF0000,
 * puis images relatives à l'ancre (apparition du joueur), fin 0xFFFFFFFF. */
__attribute__((section(".text.testpad"))) u32 smooth_pad(u32 hold)
{
    const u32 *p = S.script;
    u32 mask = 0;
    if (p) {
        u32 t = S.frame;
        if (S.anchor) {
            while (*p != 0xFFFF0000u)
                p += 2;
            p += 2;
            t = S.frame - S.anchor;
        }
        for (; *p < 0xFFFF0000u && *p <= t; p += 2)
            mask = p[1];
    }
    return hold | S.vpad | mask;
}
