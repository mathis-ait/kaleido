"""Compile le runtime ctr-smooth et produit le code.ips pour Rubis Oméga EUR v1.0.

  build.py [--code code.bin] [--counters] [--merge autre.ips] [-o sortie.ips]

Étapes : gcc (ARM, VFPv2) → ELF lié dans les marges de code.bin → binaires
.text / .data → enregistrements IPS + crochets. Chaque octet remplacé est
comparé à la valeur attendue dans code.bin d'origine (refus si différent) et
le SHA-256 de code.bin doit être celui du profil.
"""
import argparse
import hashlib
import os
import struct
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, 'tools'))
sys.path.insert(0, os.path.join(ROOT, 'proto'))
from mkpatch import read_ips, write_ips  # noqa: E402

GCC = os.environ.get('ARM_GCC_BIN', r'C:\Users\Thisma\Documents\Switch\tools-re\gcc\bin')
CODE = r'C:\Users\Thisma\Documents\Switch\tools-re\rosa-or\code.bin'
SHA = 'd587c98ac5c4dedacf9be4baf2d6b7d10169e6a63f8bc35b132002cc27bc1a37'
BASE = 0x00100000


def tool(name):
    return os.path.join(GCC, 'arm-none-eabi-' + name)


def run(*args):
    r = subprocess.run(args, capture_output=True, text=True)
    if r.returncode:
        sys.exit('échec : %s\n%s%s' % (' '.join(args), r.stdout, r.stderr))
    return r.stdout


def branch(src, dst, link=False):
    off = ((dst - (src + 8)) >> 2) & 0xFFFFFF
    return struct.pack('<I', (0xEB000000 if link else 0xEA000000) | off)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--code', default=CODE)
    ap.add_argument('--counters', action='store_true', help='ajoute les compteurs de trace de la phase 0')
    ap.add_argument('--test-input', action='store_true', help='injection de boutons via S.vpad (essais automatisés)')
    ap.add_argument('--script', help='partie scriptee compilee (essais deterministes), implique --test-input')
    ap.add_argument('--snap', type=int, default=1700, help='image de l instantane (avec --script)')
    ap.add_argument('--enabled', type=int, default=1, help='lissage actif au demarrage (avec --script)')
    ap.add_argument('--merge', action='append', default=[])
    ap.add_argument('-o', default=os.path.join(HERE, 'build', 'code.ips'))
    a = ap.parse_args()

    code = open(a.code, 'rb').read()
    if hashlib.sha256(code).hexdigest() != SHA:
        sys.exit('code.bin inattendu (profil : Rubis Oméga EUR v1.0)')

    out = os.path.join(HERE, 'build')
    os.makedirs(out, exist_ok=True)
    defines = []
    if a.script:
        a.test_input = True
        sys.path.insert(0, os.path.join(ROOT, 'tools'))
        from replay import parse_script2
        absolute, relative = parse_script2(a.script)
        lines = ['/* genere par build.py depuis %s */' % os.path.basename(a.script),
                 'static const u32 test_script[] = {']
        lines += ['    %du, 0x%xu,' % (f, m) for f, m in absolute]
        lines += ['    0xFFFF0000u, 0u, /* ancre : apparition du joueur */']
        lines += ['    %du, 0x%xu,' % (f, m) for f, m in relative]
        lines += ['    0xFFFFFFFFu, 0u,', '};',
                  '#define TEST_SNAP_REL %du' % a.snap,
                  '#define TEST_ENABLED %du' % a.enabled,
                  '#define TEST_SNAP_A 0x08C55E64u /* gfl MT19937 */',
                  '#define TEST_SNAP_ALEN 2500u', '']
        with open(os.path.join(out, 'test_script.h'), 'w', encoding='utf-8', newline='') as h:
            h.write(chr(10).join(lines))
        defines = ['-DTEST_SCRIPT', '-I', HERE]
    cflags = ['-marm', '-mcpu=mpcore', '-mfpu=vfp', '-mfloat-abi=hard', '-Os', '-ffreestanding',
              '-fno-builtin', '-nostdlib', '-fno-unwind-tables', '-fno-asynchronous-unwind-tables',
              '-fomit-frame-pointer', '-ffunction-sections', '-Wall', '-Wextra']
    run(tool('gcc'), *cflags, *defines, '-c', os.path.join(HERE, 'smooth.c'), '-o', os.path.join(out, 'smooth.o'))
    run(tool('gcc'), *cflags, '-c', os.path.join(HERE, 'hooks.S'), '-o', os.path.join(out, 'hooks.o'))
    elf = os.path.join(out, 'ctr-smooth.elf')
    run(tool('ld'), '-T', os.path.join(HERE, 'link.ld'), '--gc-sections', '-e', 'hook_gate',
        '-u', 'hook_camera', '-u', 'hook_h3d', '-u', 'hook_nwmesh', '-u', 'smooth_end', '-u', 'S',
        *(['-u', 'hook_pad'] if a.test_input else []),
        os.path.join(out, 'hooks.o'), os.path.join(out, 'smooth.o'), '-o', elf)
    run(tool('objcopy'), '-O', 'binary', '-j', '.text', elf, os.path.join(out, 'text.bin'))
    syms = {}
    for line in run(tool('nm'), elf).splitlines():
        p = line.split()
        if len(p) == 3:
            syms[p[2]] = int(p[0], 16)
    text = open(os.path.join(out, 'text.bin'), 'rb').read()
    t0 = 0x00579610
    if t0 + len(text) > 0x0057A000:
        sys.exit('runtime trop gros : .text %d / 2544' % len(text))
    if not (0x006AE620 <= syms['S'] and syms['S'] + 0x40 <= 0x006AEFF0):
        sys.exit('etat hors de la marge du .bss')

    def orig(addr, n=4):
        return code[addr - BASE:addr - BASE + n]

    # Instructions remplacées et leur valeur d'origine (contrôle anti-mauvaise version).
    hooks = [
        (0x0010E530, 'ldrb r0,[r4,#0xd]', bytes.fromhex('0d00d4e5'), branch(0x0010E530, syms['hook_gate'], True)),
        (0x0010E534, 'rsb r1,r6,#0', bytes.fromhex('001066e2'), bytes.fromhex('000050e3')),  # cmp r0,#0
        (0x0010E538, 'tst r0,r1', bytes.fromhex('010010e1'), bytes.fromhex('00f020e3')),     # nop
        (0x0010E61C, 'nop', bytes.fromhex('00f020e3'), branch(0x0010E61C, syms['smooth_end'], True)),
        (0x00375EA8, 'push {r0-r11,lr}', bytes.fromhex('ff4f2de9'), branch(0x00375EA8, syms['hook_camera'])),
        (0x0039B338, 'push {r3-r11,lr}', bytes.fromhex('f84f2de9'), branch(0x0039B338, syms['hook_h3d'])),
        (0x002EC354, 'push {r4-r8,lr}', bytes.fromhex('f0412de9'), branch(0x002EC354, syms['hook_nwmesh'])),
    ]
    if a.test_input:
        hooks.append((0x0036FB34, 'add r1,r4,#0x98', bytes.fromhex('981084e2'), branch(0x0036FB34, syms['hook_pad'], True)))
    recs = []
    for m in a.merge:
        recs += read_ips(m)
    for addr, what, want, new in hooks:
        if orig(addr) != want:
            sys.exit('0x%08x : %s attendu (%s), trouvé %s' % (addr, what, want.hex(), orig(addr).hex()))
        recs.append((addr - BASE, new))
    if any(orig(t0, len(text))):
        sys.exit('la marge de .text n est pas vide')
    recs.append((t0 - BASE, text))
    if a.counters:
        import instrument
        recs += instrument.records(draw60=False)
    os.makedirs(os.path.dirname(os.path.abspath(a.o)), exist_ok=True)
    write_ips(a.o, recs)
    print('runtime : .text %d octets (marge %d), etat a 0x%08x' % (len(text), 0x0057A000 - t0 - len(text), syms['S']))
    for k in ('hook_gate', 'hook_camera', 'hook_h3d', 'hook_nwmesh', 'smooth_gate', 'smooth_end', 'S') + (('hook_pad',) if a.test_input else ()):
        print('  %-14s 0x%08x' % (k, syms[k]))
    print('->', a.o)


if __name__ == '__main__':
    main()
