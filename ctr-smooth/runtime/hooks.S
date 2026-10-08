@ Trampolines ctr-smooth (ARM, ARMv6K). Chaque crochet d'entrée remplace la
@ première instruction (un push, indépendant de la position) de la fonction
@ visée par « b hook_xxx » ; le trampoline appelle le C en préservant r0-r3, r12,
@ lr et d0-d7 (paramètres flottants possibles en ABI hard-float), rejoue
@ l'instruction déplacée puis revient à l'instruction suivante.

    .syntax unified
    .arm
    .section .text.hooks, "ax"

@ 0x0010E530 « ldrb r0,[r4,#0xd] » devient « bl hook_gate » ; r4 = gestionnaire
@ d'images, r6 = 1 si update() a tourné sur cette image. Retour : r0 = sauter.
    .global hook_gate
hook_gate:
    mov     r0, r4
    mov     r1, r6
    b       smooth_gate

@ 0x00375EA8 (copie de la vue avant le parcours de la file de rendu) :
@ la caméra est le 4e argument de pile, [sp+0xC] à l'entrée.
    .global hook_camera
hook_camera:
    push    {r0-r3, r12, lr}
    vpush   {d0-d7}
    ldr     r0, [sp, #(24 + 64 + 0xC)]
    bl      smooth_camera
    vpop    {d0-d7}
    pop     {r0-r3, r12, lr}
    push    {r0-r11, lr}
    b       0x00375EAC

@ 0x0039B338 (rendu d'un maillage H3D) : r0 = modèle.
    .global hook_h3d
hook_h3d:
    push    {r0-r3, r12, lr}
    vpush   {d0-d7}
    bl      smooth_h3d
    vpop    {d0-d7}
    pop     {r0-r3, r12, lr}
    push    {r3-r11, lr}
    b       0x0039B33C

@ 0x002EC354 (nw::gfx MeshRenderer::RenderMesh) : r2 = modèle.
    .global hook_nwmesh
hook_nwmesh:
    push    {r0-r3, r12, lr}
    vpush   {d0-d7}
    mov     r0, r2
    bl      smooth_nwmodel
    vpop    {d0-d7}
    pop     {r0-r3, r12, lr}
    push    {r4-r8, lr}
    b       0x002EC358

@ Builds de test uniquement (build.py --test-input) : 0x0036FB34, juste après
@ nn::hid::PadReader::ReadLatest dans gfl::ui::CTR_DeviceManager::Main, devient
@ « bl hook_pad ». r4 = gestionnaire, mot « boutons tenus » en [r4+0x78].
@ Ajoute S.vpad (écrit par le stub GDB) puis rejoue « add r1, r4, #0x98 ».
    .section .text.testpad, "ax"
    .global hook_pad
hook_pad:
    push    {r4, lr}
    ldr     r0, [r4, #0x78]
    bl      smooth_pad
    str     r0, [r4, #0x78]
    add     r1, r4, #0x98
    pop     {r4, pc}
