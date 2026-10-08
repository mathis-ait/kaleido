0010e330  14009fe5  ldr      r0, [pc, #0x14]   ; =0x0062f7c0
0010e334  14109fe5  ldr      r1, [pc, #0x14]   ; =0x0058b82c
0010e338  0020a0e3  mov      r2, #0
0010e33c  000090e5  ldr      r0, [r0]
0010e340  001091e5  ldr      r1, [r1]
0010e344  040090e5  ldr      r0, [r0, #4]
0010e348  053e00ea  b        #0x11db64
0010e34c  c0f76200  rsbeq    pc, r2, r0, asr #15
0010e350  2cb85800  subseq   fp, r8, ip, lsr #16
0010e354  f04f2de9  push     {r4, r5, r6, r7, r8, sb, sl, fp, lr}
0010e358  0040a0e1  mov      r4, r0
0010e35c  0100a0e3  mov      r0, #1
0010e360  0080a0e3  mov      r8, #0
0010e364  028b2ded  vpush    {d8}
0010e368  0cd04de2  sub      sp, sp, #0xc
0010e36c  c78a9fed  vldr     s16, [pc, #0x31c]
0010e370  00008de5  str      r0, [sp]
0010e374  0d00d4e5  ldrb     r0, [r4, #0xd]
0010e378  000050e3  cmp      r0, #0
0010e37c  0400000a  beq      #0x10e394
0010e380  0e00d4e5  ldrb     r0, [r4, #0xe]
0010e384  010030e2  eors     r0, r0, #1
0010e388  0060a003  moveq    r6, #0
0010e38c  0e00c4e5  strb     r0, [r4, #0xe]
0010e390  0800000a  beq      #0x10e3b8
0010e394  0160a0e3  mov      r6, #1
0010e398  0000a0e1  mov      r0, r0
0010e39c  00f020e3  nop      
0010e3a0  00f020e3  nop      
0010e3a4  0000a0e3  mov      r0, #0
0010e3a8  000050e3  cmp      r0, #0
0010e3ac  00f020e3  nop      
0010e3b0  5e00001a  bne      #0x10e530
0010e3b4  050000ea  b        #0x10e3d0
0010e3b8  00f020e3  nop      
0010e3bc  0000a0e3  mov      r0, #0
0010e3c0  000050e3  cmp      r0, #0
0010e3c4  00f020e3  nop      
0010e3c8  5800001a  bne      #0x10e530
0010e3cc  3e0000ea  b        #0x10e4cc
0010e3d0  180094e5  ldr      r0, [r4, #0x18]
0010e3d4  000090e5  ldr      r0, [r0]
0010e3d8  000050e3  cmp      r0, #0
0010e3dc  0b00001a  bne      #0x10e410
0010e3e0  145094e5  ldr      r5, [r4, #0x14]
0010e3e4  0500a0e1  mov      r0, r5
0010e3e8  f15300eb  bl       #0x1233b4
0010e3ec  020050e3  cmp      r0, #2
0010e3f0  00f020e3  nop      
0010e3f4  0500001a  bne      #0x10e410
0010e3f8  0500a0e1  mov      r0, r5
0010e3fc  f05300eb  bl       #0x1233c4
0010e400  001090e5  ldr      r1, [r0]
0010e404  202091e5  ldr      r2, [r1, #0x20]
0010e408  141095e5  ldr      r1, [r5, #0x14]
0010e40c  32ff2fe1  blx      r2
0010e410  180094e5  ldr      r0, [r4, #0x18]
0010e414  ac2600eb  bl       #0x117ecc
0010e418  2c0094e5  ldr      r0, [r4, #0x2c]
0010e41c  00f020e3  nop      
0010e420  115200eb  bl       #0x122c6c
0010e424  300094e5  ldr      r0, [r4, #0x30]
0010e428  00f020e3  nop      
0010e42c  707e00eb  bl       #0x12ddf4
0010e430  145094e5  ldr      r5, [r4, #0x14]
0010e434  000055e3  cmp      r5, #0
0010e438  2000000a  beq      #0x10e4c0
0010e43c  0500a0e1  mov      r0, r5
0010e440  188085e5  str      r8, [r5, #0x18]
0010e444  da5300eb  bl       #0x1233b4
0010e448  020050e3  cmp      r0, #2
0010e44c  0100a003  moveq    r0, #1
0010e450  18008505  streq    r0, [r5, #0x18]
0010e454  0500a0e1  mov      r0, r5
0010e458  474200eb  bl       #0x11ed7c
0010e45c  020050e3  cmp      r0, #2
0010e460  0070a0e1  mov      r7, r0
0010e464  03005713  cmpne    r7, #3
0010e468  1100001a  bne      #0x10e4b4
0010e46c  0500a0e1  mov      r0, r5
0010e470  d35300eb  bl       #0x1233c4
0010e474  000050e3  cmp      r0, #0
0010e478  00f020e3  nop      
0010e47c  0c00000a  beq      #0x10e4b4
0010e480  001090e5  ldr      r1, [r0]
0010e484  149095e5  ldr      sb, [r5, #0x14]
0010e488  181091e5  ldr      r1, [r1, #0x18]
0010e48c  31ff2fe1  blx      r1
0010e490  020050e3  cmp      r0, #2
0010e494  0c00c935  strblo   r0, [sb, #0xc]
0010e498  0300003a  blo      #0x10e4ac
0010e49c  0020a0e3  mov      r2, #0
0010e4a0  0210a0e1  mov      r1, r2
0010e4a4  0200a0e1  mov      r0, r2
0010e4a8  0000a0e1  mov      r0, r0
0010e4ac  140095e5  ldr      r0, [r5, #0x14]
0010e4b0  048a80ed  vstr     s16, [r0, #0x10]
0010e4b4  000057e3  cmp      r7, #0
0010e4b8  0800a001  moveq    r0, r8
0010e4bc  0000000a  beq      #0x10e4c4
0010e4c0  0100a0e3  mov      r0, #1
0010e4c4  00008de5  str      r0, [sp]
0010e4c8  180000ea  b        #0x10e530
0010e4cc  145094e5  ldr      r5, [r4, #0x14]
0010e4d0  000055e3  cmp      r5, #0
0010e4d4  1500000a  beq      #0x10e530
0010e4d8  180095e5  ldr      r0, [r5, #0x18]
0010e4dc  000050e3  cmp      r0, #0
0010e4e0  1200000a  beq      #0x10e530
0010e4e4  0500a0e1  mov      r0, r5
0010e4e8  b15300eb  bl       #0x1233b4
0010e4ec  020050e3  cmp      r0, #2
0010e4f0  00f020e3  nop      
0010e4f4  0d00001a  bne      #0x10e530
0010e4f8  0500a0e1  mov      r0, r5
0010e4fc  b05300eb  bl       #0x1233c4
0010e500  0070a0e1  mov      r7, r0
0010e504  000090e5  ldr      r0, [r0]
0010e508  0510a0e1  mov      r1, r5
0010e50c  1c2090e5  ldr      r2, [r0, #0x1c]
0010e510  0700a0e1  mov      r0, r7
0010e514  32ff2fe1  blx      r2
0010e518  010050e3  cmp      r0, #1
0010e51c  0300001a  bne      #0x10e530
0010e520  0310a0e3  mov      r1, #3
0010e524  0500a0e1  mov      r0, r5
0010e528  e94100eb  bl       #0x11ecd4
0010e52c  048087e5  str      r8, [r7, #4]
0010e530  0d00d4e5  ldrb     r0, [r4, #0xd]
0010e534  001066e2  rsb      r1, r6, #0
0010e538  010010e1  tst      r0, r1
0010e53c  0400001a  bne      #0x10e554
0010e540  1c0094e5  ldr      r0, [r4, #0x1c]
0010e544  fe11d0e5  ldrb     r1, [r0, #0x1fe]
0010e548  000051e3  cmp      r1, #0
0010e54c  fe81c015  strbne   r8, [r0, #0x1fe]
0010e550  0300000a  beq      #0x10e564
0010e554  00009de5  ldr      r0, [sp]
0010e558  0cd08de2  add      sp, sp, #0xc
0010e55c  028bbdec  vpop     {d8}
0010e560  f08fbde8  pop      {r4, r5, r6, r7, r8, sb, sl, fp, pc}
0010e564  00f020e3  nop      
0010e568  280000ef  svc      #0x28
0010e56c  0050a0e1  mov      r5, r0
0010e570  140094e5  ldr      r0, [r4, #0x14]
0010e574  0170a0e1  mov      r7, r1
0010e578  000050e3  cmp      r0, #0
0010e57c  2600000a  beq      #0x10e61c
0010e580  0c019fe5  ldr      r0, [pc, #0x10c]   ; =0x0063106c
0010e584  0710a0e3  mov      r1, #7
0010e588  046090e5  ldr      r6, [r0, #4]
0010e58c  0600a0e1  mov      r0, r6
0010e590  830c00eb  bl       #0x1117a4
0010e594  140094e5  ldr      r0, [r4, #0x14]
0010e598  0a10d0e5  ldrb     r1, [r0, #0xa]
0010e59c  000051e3  cmp      r1, #0
0010e5a0  cf41001b  blne     #0x11ece4
0010e5a4  00f020e3  nop      
0010e5a8  00f020e3  nop      
0010e5ac  6b3900eb  bl       #0x11cb60
0010e5b0  00f020e3  nop      
0010e5b4  00f020e3  nop      
0010e5b8  885100eb  bl       #0x122be0
0010e5bc  00f020e3  nop      
0010e5c0  00f020e3  nop      
0010e5c4  a85000eb  bl       #0x12286c
0010e5c8  00f020e3  nop      
0010e5cc  00f020e3  nop      
0010e5d0  825100eb  bl       #0x122be0
0010e5d4  00f020e3  nop      
0010e5d8  00f020e3  nop      
0010e5dc  8b5000eb  bl       #0x122810
0010e5e0  0110a0e3  mov      r1, #1
0010e5e4  0600a0e1  mov      r0, r6
0010e5e8  0000a0e1  mov      r0, r0
0010e5ec  00f020e3  nop      
0010e5f0  00f020e3  nop      
0010e5f4  55cf00eb  bl       #0x142350
0010e5f8  000050e3  cmp      r0, #0
0010e5fc  258a9f0d  vldreq   s16, [pc, #0x94]
0010e600  040a94ed  vldr     s0, [r4, #0x10]
0010e604  0600a0e1  mov      r0, r6
0010e608  000a28ee  vmul.f32 s0, s16, s0
0010e60c  0c3a00eb  bl       #0x11ce44
0010e610  0600a0e1  mov      r0, r6
0010e614  00f020e3  nop      
0010e618  c70b00eb  bl       #0x11153c
0010e61c  00f020e3  nop      
0010e620  280000ef  svc      #0x28
0010e624  056050e0  subs     r6, r0, r5
0010e628  0750c1e0  sbc      r5, r1, r7
0010e62c  0d10d4e5  ldrb     r1, [r4, #0xd]
0010e630  0000a0e3  mov      r0, #0
0010e634  04008de5  str      r0, [sp, #4]
0010e638  1c0094e5  ldr      r0, [r4, #0x1c]
0010e63c  000051e3  cmp      r1, #0
0010e640  54709f05  ldreq    r7, [pc, #0x54]   ; =0x0000411a
0010e644  54709f15  ldrne    r7, [pc, #0x54]   ; =0x00008235
0010e648  fc01d0e5  ldrb     r0, [r0, #0x1fc]
0010e64c  010050e3  cmp      r0, #1
0010e650  0300001a  bne      #0x10e664
0010e654  f43b00eb  bl       #0x11d62c
0010e658  070050e1  cmp      r0, r7
0010e65c  0100a083  movhi    r0, #1
0010e660  04008d85  strhi    r0, [sp, #4]
0010e664  1c0094e5  ldr      r0, [r4, #0x1c]
0010e668  fd01d0e5  ldrb     r0, [r0, #0x1fd]
0010e66c  010050e3  cmp      r0, #1
0010e670  0b00000a  beq      #0x10e6a4
0010e674  04009de5  ldr      r0, [sp, #4]
0010e678  000050e3  cmp      r0, #0
0010e67c  b4ffff0a  beq      #0x10e554
0010e680  1c0094e5  ldr      r0, [r4, #0x1c]
0010e684  0110a0e3  mov      r1, #1
0010e688  fe11c0e5  strb     r1, [r0, #0x1fe]
0010e68c  b0ffffea  b        #0x10e554
