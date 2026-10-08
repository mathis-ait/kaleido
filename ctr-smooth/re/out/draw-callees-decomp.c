// ===== FUN_001117a4 @ 001117a4 (548 octets)

void FUN_001117a4(int param_1,uint param_2)

{
  undefined4 *puVar1;
  int iVar2;
  int *piVar3;
  undefined4 uVar4;
  undefined4 uVar5;
  int iVar6;
  undefined4 uVar7;
  uint uVar8;
  uint uVar9;
  undefined4 *puVar10;
  int *piVar11;
  int iVar12;
  uint uVar13;
  undefined4 local_48;
  undefined4 uStack_44;
  undefined4 uStack_40;
  undefined4 uStack_3c;
  int iStack_34;
  uint uStack_30;
  int local_2c;
  int iStack_28;
  
  iVar12 = *(int *)(param_1 + 0x178);
  iVar6 = param_1 + 0x160;
  local_2c = param_1 + 0x130;
  iStack_34 = iVar12;
  uStack_30 = param_2;
  iStack_28 = iVar6;
  if (*(char *)(iVar12 + 0x73) != '\0') {
    FUN_0011ce58(iVar12,0,2,1);
  }
  uVar8 = 0;
  do {
    if ((param_2 & 1 << (uVar8 & 0xff)) != 0) {
      uVar13 = (uint)(uVar8 != 0 && uVar8 != 2);
      if (uVar8 == 4) {
        uVar9 = *(uint *)(iVar12 + 0x80);
      }
      else if (uVar8 < 3) {
        uVar9 = *(uint *)(iVar12 + (uint)*(byte *)(DAT_001119cc + uVar8) * 4 + 0x80);
      }
      else {
        uVar9 = *(uint *)(iVar12 + 0x84);
      }
      if (uVar9 != 0xffffffff) {
        piVar11 = *(int **)(iVar12 + 0xc);
        if (uVar9 < (uint)piVar11[1]) {
          FUN_00130014(*(undefined4 *)(*piVar11 + uVar9 * 0x1c));
          piVar11[4] = uVar9;
        }
        if (iVar6 == 0) {
          uVar4 = 1;
          uVar9 = *(uint *)(iVar12 + uVar8 * 4 + 0x28);
          piVar11 = *(int **)(iVar12 + 8);
          puVar1 = (undefined4 *)(local_2c + uVar13 * 0x18);
          if ((uVar9 < *(ushort *)(piVar11 + 1)) &&
             (iVar2 = *piVar11 + uVar9 * 0x40, *(char *)(iVar2 + 4) != '\0')) {
            piVar3 = *(int **)(iVar2 + 0x1c);
            if (piVar3 != (int *)0x0) {
              (**(code **)(*piVar3 + 0x10))();
              FUN_001525e0();
            }
            local_48 = *puVar1;
            uStack_44 = puVar1[1];
            uStack_40 = puVar1[2];
            uStack_3c = puVar1[3];
            uVar13 = VectorFloatToUnsigned(puVar1[5],3);
            uVar5 = 1;
            uVar7 = 1;
            iVar2 = *piVar11 + uVar9 * 0x40;
            goto LAB_0011199c;
          }
        }
        else {
          piVar11 = *(int **)(iVar12 + 8);
          uVar9 = *(uint *)(iVar12 + uVar8 * 4 + 0x28);
          puVar10 = (undefined4 *)(local_2c + uVar13 * 0x18);
          puVar1 = (undefined4 *)(iVar6 + uVar13 * 0xc);
          if ((uVar9 < *(ushort *)(piVar11 + 1)) &&
             (iVar2 = *piVar11 + uVar9 * 0x40, *(char *)(iVar2 + 4) != '\0')) {
            piVar3 = *(int **)(iVar2 + 0x1c);
            if (piVar3 != (int *)0x0) {
              (**(code **)(*piVar3 + 0x10))();
              FUN_001525e0();
            }
            local_48 = *puVar10;
            uStack_44 = puVar10[1];
            uStack_40 = puVar10[2];
            uStack_3c = puVar10[3];
            uVar13 = VectorFloatToUnsigned(puVar10[5],3);
            uVar4 = puVar1[2];
            uVar5 = *puVar1;
            uVar7 = puVar1[1];
            iVar2 = *piVar11 + uVar9 * 0x40;
LAB_0011199c:
            FUN_0011cdd0(iVar2,uVar5,&local_48,uVar7,uVar4,uVar13 & 0xff);
          }
        }
      }
    }
    uVar8 = uVar8 + 1;
    if (2 < uVar8) {
      FUN_0013e9cc(iVar12,4);
      *(uint *)(iVar12 + 0x74) = param_2;
      *(undefined1 *)(iVar12 + 0x73) = 1;
      return;
    }
  } while( true );
}


// ===== FUN_0011cb60 @ 0011cb60 (536 octets)

void FUN_0011cb60(void)

{
  char cVar1;
  int iVar2;
  uint uVar3;
  int *piVar4;
  int iVar5;
  uint uVar6;
  int iVar7;
  int iVar8;
  uint uVar9;
  uint uVar10;
  int iVar11;
  undefined4 local_28;
  undefined4 local_24;
  
  iVar2 = *DAT_0011cb6c;
  iVar8 = 0;
  do {
    FUN_0014bf90(*(undefined4 *)(iVar2 + iVar8 * 4 + 0x38),*(undefined4 *)(iVar2 + 0x34));
    iVar8 = iVar8 + 1;
  } while (iVar8 < 2);
  uVar6 = 0;
  do {
    cVar1 = *(char *)(*(int *)(iVar2 + uVar6 * 4 + 0x38) + 0x48);
    if (cVar1 == '\x01' || cVar1 == '\x03') {
      if (*(int *)(iVar2 + 0x40) == 0) {
        return;
      }
      local_28 = *(undefined4 *)(iVar2 + 0x20);
      local_24 = *(undefined4 *)(iVar2 + 0x44);
      iVar8 = (**(code **)(iVar2 + 0x40))(&local_28);
      if (iVar8 != 0) {
        *(undefined4 *)(iVar2 + 0x40) = 0;
        *(undefined4 *)(iVar2 + 0x44) = 0;
      }
      return;
    }
    uVar6 = uVar6 + 1;
  } while ((int)uVar6 < 2);
  if (*(int *)(iVar2 + 0x40) != 0) {
    *(undefined4 *)(iVar2 + 0x40) = 0;
  }
  if (*(int *)(iVar2 + 0x44) != 0) {
    *(undefined4 *)(iVar2 + 0x44) = 0;
  }
  if (*(char *)(*(int *)(iVar2 + 0x38) + 0x48) != '\0') {
    return;
  }
  if (*(char *)(iVar2 + 0x1c) == '\0') {
    return;
  }
  uVar3 = *(uint *)(iVar2 + 0x28);
  if (uVar3 != 0xffff) {
    piVar4 = *(int **)(**(int **)(iVar2 + 4) + 0x10);
    uVar6 = piVar4[1];
    if ((uVar3 < uVar6) && (*piVar4 + uVar3 * 0x48 != 0)) {
      FUN_00138768();
    }
  }
  if (*(int *)(iVar2 + 0x20) != 0) {
    uVar3 = 0;
    if (*(int *)(iVar2 + 0x24) != 0) {
      do {
        iVar8 = *(int *)(*(int *)(iVar2 + 0x20) + uVar3 * 8);
        if (iVar8 != 0) {
          iVar11 = *(int *)(iVar8 + 4);
          iVar7 = *(int *)(iVar11 + 0xc0);
          if (iVar7 == 0) {
            uVar6 = *(uint *)(iVar11 + 0xc4);
          }
          iVar5 = *(int *)(**(int **)(iVar2 + 4) + 0x14);
          if ((iVar7 != 0 || uVar6 != 0) &&
             (uVar6 = *(uint *)(iVar11 + 0xc4), uVar6 != 0 && iVar7 != 0)) {
            *(short *)(iVar5 + 6) = *(short *)(iVar5 + 6) + -1;
            iVar5 = *(int *)(*(int *)(iVar8 + 4) + 0xc0);
            iVar7 = *(int *)(*(int *)(iVar8 + 4) + 0xc4);
            *(int *)(*(int *)(iVar5 + 4) + 0xc4) = iVar7;
            *(int *)(*(int *)(iVar7 + 4) + 0xc0) = iVar5;
            uVar6 = 0;
            *(undefined4 *)(*(int *)(iVar8 + 4) + 0xc4) = 0;
            *(undefined4 *)(*(int *)(iVar8 + 4) + 0xc0) = 0;
          }
          uVar10 = *(uint *)(iVar11 + 0xac);
          uVar9 = 0;
          if (uVar10 != 0) {
            do {
              FUN_0014d43c(iVar11,uVar9);
              uVar9 = uVar9 + 1;
            } while (uVar9 < uVar10);
          }
          FUN_00138eb8(iVar11);
        }
        uVar3 = uVar3 + 1;
      } while (uVar3 < *(uint *)(iVar2 + 0x24));
      if (*(int *)(iVar2 + 0x20) == 0) goto LAB_0012b0c0;
    }
    FUN_002ff8c0(*(undefined4 *)(iVar2 + 0x20));
    *(undefined4 *)(iVar2 + 0x20) = 0;
  }
LAB_0012b0c0:
  *(undefined4 *)(*(int *)(iVar2 + 0x38) + 0x60) = 0;
  *(undefined1 *)(iVar2 + 0x1c) = 0;
  return;
}


// ===== FUN_00122be0 @ 00122be0 (12 octets)

undefined4 FUN_00122be0(void)

{
  return *(undefined4 *)(DAT_00122bec + 0x54);
}


// ===== FUN_0012286c @ 0012286c (84 octets)

undefined4 FUN_0012286c(int param_1,int param_2)

{
  int iVar1;
  int extraout_r1;
  bool bVar2;
  
  iVar1 = *(int *)(param_1 + 0x1c);
  if (iVar1 != 0) {
    param_2 = *(int *)(param_1 + 4);
  }
  if (iVar1 != 0 && param_2 != -1) {
    FUN_0012abc4(iVar1,DAT_001228c0);
    bVar2 = *(int *)(param_1 + 0x24) != 0;
    iVar1 = extraout_r1;
    if (bVar2) {
      iVar1 = *(int *)(param_1 + 4);
    }
    if ((bVar2 && iVar1 != -1) && (iVar1 = FUN_0012f990(), iVar1 != 0)) {
      *(undefined4 *)(param_1 + 4) = 0xffffffff;
    }
  }
  return 0;
}


// ===== FUN_00122810 @ 00122810 (88 octets)

void FUN_00122810(int param_1)

{
  int iVar1;
  undefined4 uVar2;
  bool bVar3;
  
  bVar3 = *(int *)(param_1 + 0x1c) != 0;
  iVar1 = 0;
  if (bVar3) {
    iVar1 = *(int *)(param_1 + 4);
  }
  if (bVar3 && iVar1 != -1) {
    uVar2 = FUN_0012f94c(*(undefined4 *)(param_1 + 0x10),1);
    FUN_0012aa3c(*(undefined4 *)(param_1 + 0x1c),*(undefined4 *)(param_1 + 0x10),1,uVar2,0,
                 DAT_00122868);
  }
  return;
}


// ===== FUN_00142350 @ 00142350 (48 octets)

undefined4 FUN_00142350(void)

{
  undefined4 uVar1;
  
  if ((*(char *)(DAT_00142380 + 0x84) != '\0') || (*(float *)(DAT_00142380 + 0x80) <= DAT_00142384))
  {
    uVar1 = 0;
  }
  else {
    uVar1 = 1;
  }
  return uVar1;
}


// ===== FUN_0011ce44 @ 0011ce44 (20 octets)

void FUN_0011ce44(int param_1)

{
  int iVar1;
  int iVar2;
  undefined4 uVar3;
  undefined4 uVar4;
  uint uVar5;
  uint uVar6;
  int iVar7;
  int *piVar8;
  int iVar9;
  float in_s0;
  float fVar10;
  undefined4 auStack_64 [2];
  uint auStack_5c [10];
  int iStack_34;
  undefined4 uStack_30;
  undefined4 uStack_2c;
  undefined4 uStack_28;
  
  iVar1 = DAT_0011d1e0;
  iVar2 = *(int *)(param_1 + 0x178);
  uStack_28 = 1;
  uStack_2c = 2;
  uStack_30 = 0;
  if (*(char *)(iVar2 + 0x73) != '\0') {
    uVar5 = 0;
    fVar10 = DAT_0011d1dc;
    iStack_34 = iVar2;
    if (*(int *)(iVar2 + 0x6c) != 0) {
      do {
        if ((*(uint *)(iVar2 + 0x74) & 1 << (uVar5 & 0xff)) != 0) {
          if (uVar5 == 2) {
            if ((*(char *)(iVar2 + 0x71) == '\0') || (*(char *)(iVar2 + 0x72) != '\0'))
            goto LAB_0011cff0;
            if (in_s0 != fVar10) goto LAB_0011ced8;
LAB_0011d010:
            if ((*(char *)(iVar2 + 0xc0) != '\0') || (in_s0 == fVar10)) {
LAB_0011d028:
              *(undefined4 *)(iVar2 + 0xa8) = *(undefined4 *)(iVar2 + 0x94);
            }
          }
          else {
LAB_0011ced8:
            if (uVar5 == 4) {
              uVar6 = *(uint *)(iVar2 + 0x80);
            }
            else if (uVar5 < 3) {
              uVar6 = *(uint *)(iVar2 + (uint)*(byte *)(iVar1 + uVar5) * 4 + 0x80);
            }
            else {
              uVar6 = *(uint *)(iVar2 + 0x84);
            }
            if (uVar6 != 0xffffffff) {
              piVar8 = *(int **)(iVar2 + 0xc);
              if (uVar6 < (uint)piVar8[1]) {
                FUN_00130014(*(undefined4 *)(*piVar8 + uVar6 * 0x1c));
                piVar8[4] = uVar6;
              }
              iVar7 = iVar2 + uVar5 * 4;
              if (((uint)*(ushort *)(*(int **)(iVar2 + 8) + 1) <= *(uint *)(iVar7 + 0x28)) ||
                 (iVar9 = **(int **)(iVar2 + 8) + *(uint *)(iVar7 + 0x28) * 0x40,
                 *(char *)(iVar9 + 4) == '\0')) {
                iVar9 = 0;
              }
              FUN_00157fbc();
              FUN_001298bc(iVar2,uVar5);
              if (((2 < *(uint *)(iVar2 + 0x6c)) && (uVar5 == 0)) &&
                 ((*(char *)(iVar2 + 0x71) == '\0' ||
                  ((*(char *)(iVar2 + 0x72) != '\0' || (in_s0 == fVar10)))))) {
                FUN_001298bc(iVar2,2,0);
              }
              if (*(uint *)(iVar7 + 0x94) < (uint)(*(int **)(iVar2 + 4))[1]) {
                FUN_00129dbc(**(int **)(iVar2 + 4) + *(uint *)(iVar7 + 0x94) * 0x2c,iVar9);
              }
              *(undefined4 *)(iVar7 + 0xa0) = *(undefined4 *)(iVar7 + 0x94);
            }
LAB_0011cff0:
            if (uVar5 == 2) {
              if ((*(char *)(iVar2 + 0x71) != '\0') && (*(char *)(iVar2 + 0x72) == '\0'))
              goto LAB_0011d010;
              goto LAB_0011d028;
            }
          }
        }
        uVar5 = uVar5 + 1;
      } while (uVar5 < *(uint *)(iVar2 + 0x6c));
    }
    auStack_5c[0] = 0;
    auStack_5c[1] = uStack_2c;
    auStack_5c[2] = uStack_28;
    FUN_0013e9cc(iVar2,4);
    iVar7 = 0;
    do {
      uVar5 = auStack_5c[iVar7];
      if ((*(uint *)(iVar2 + 0x74) & 1 << (uVar5 & 0xff)) != 0) {
        if (uVar5 == 4) {
          iVar9 = *(int *)(iVar2 + 0x80);
        }
        else if (uVar5 < 3) {
          iVar9 = *(int *)(iVar2 + (uint)*(byte *)(iVar1 + uVar5) * 4 + 0x80);
        }
        else {
          iVar9 = *(int *)(iVar2 + 0x84);
        }
        if ((iVar9 != -1) &&
           ((uVar5 != 2 ||
            (((*(char *)(iVar2 + 0x71) != '\0' && (*(char *)(iVar2 + 0x72) == '\0')) &&
             (in_s0 != fVar10)))))) {
          piVar8 = *(int **)(iVar2 + 0xc);
          if (uVar5 == 4) {
            uVar5 = *(uint *)(iVar2 + 0x80);
          }
          else if (uVar5 < 3) {
            uVar5 = *(uint *)(iVar2 + (uint)*(byte *)(iVar1 + uVar5) * 4 + 0x80);
          }
          else {
            uVar5 = *(uint *)(iVar2 + 0x84);
          }
          if (uVar5 < (uint)piVar8[1]) {
            iVar9 = uVar5 * 7;
            FUN_00130014(*(undefined4 *)(*piVar8 + uVar5 * 0x1c));
            FUN_001301b8(DAT_0011d1e4,auStack_5c + 3);
            FUN_001301b8(DAT_0011d1e8,auStack_64);
            if (piVar8[4] != -1) {
              FUN_00130014(*(undefined4 *)(*piVar8 + piVar8[4] * 0x1c));
            }
            FUN_001301b8(DAT_0011d1e4,DAT_0011d1ec);
            FUN_001301b8(DAT_0011d1e8,DAT_0011d1f0);
            FUN_00131414(*(undefined4 *)(*piVar8 + iVar9 * 4),0,auStack_5c[3],0,auStack_64[0],0,0);
          }
        }
      }
      iVar7 = iVar7 + 1;
    } while (iVar7 < 3);
    *(undefined4 *)(iVar2 + 0xb8) = *(undefined4 *)(iVar2 + 0x80);
    if (*(char *)(iVar2 + 0x72) == '\x01') {
      *(undefined1 *)(iVar2 + 0x72) = 2;
    }
    uVar3 = FUN_00145d44();
    uVar4 = FUN_00145d54();
    thunk_FUN_001487b8(uVar4,uVar3);
    *(undefined1 *)(iVar2 + 0x73) = 0;
  }
  return;
}


// ===== FUN_0011153c @ 0011153c (244 octets)

void FUN_0011153c(int param_1)

{
  byte bVar1;
  char cVar2;
  char *pcVar3;
  undefined4 uVar4;
  undefined4 uVar5;
  undefined4 uVar6;
  
  pcVar3 = *(char **)(param_1 + 0x178);
  bVar1 = pcVar3[2];
  if ((bVar1 & 0xc) != 0) {
    pcVar3[2] = bVar1 & 0xf0 | (byte)(((uint)bVar1 << 0x1a) >> 0x1e);
  }
  if ((*pcVar3 == '\0') || ((pcVar3[2] & 3U) != 0)) {
    uVar4 = *(undefined4 *)(pcVar3 + 0xa0);
    uVar5 = *(undefined4 *)(pcVar3 + 0xa4);
    uVar6 = *(undefined4 *)(pcVar3 + 0xa8);
    pcVar3[0xa0] = -1;
    pcVar3[0xa1] = -1;
    pcVar3[0xa2] = -1;
    pcVar3[0xa3] = -1;
    pcVar3[0xa4] = -1;
    pcVar3[0xa5] = -1;
    pcVar3[0xa6] = -1;
    pcVar3[0xa7] = -1;
    pcVar3[0xa8] = -1;
    pcVar3[0xa9] = -1;
    pcVar3[0xaa] = -1;
    pcVar3[0xab] = -1;
    *(undefined4 *)(pcVar3 + 0xac) = uVar4;
    *(undefined4 *)(pcVar3 + 0xb0) = uVar5;
    *(undefined4 *)(pcVar3 + 0xb4) = uVar6;
    if (*(int *)(pcVar3 + 0xb8) != -1) {
      FUN_0011d63c(*(undefined4 *)(pcVar3 + 0xc));
      FUN_0011d7e0(*(undefined4 *)(pcVar3 + 0xc));
    }
    FUN_0011d1f4(pcVar3);
  }
  else {
    if (*(char *)(*(int *)(pcVar3 + 0xc) + 0x14) != '\0') {
      FUN_0011d7e0();
    }
    FUN_0011d1f4(pcVar3);
    uVar4 = *(undefined4 *)(pcVar3 + 0xa0);
    uVar5 = *(undefined4 *)(pcVar3 + 0xa4);
    uVar6 = *(undefined4 *)(pcVar3 + 0xa8);
    pcVar3[0xa0] = -1;
    pcVar3[0xa1] = -1;
    pcVar3[0xa2] = -1;
    pcVar3[0xa3] = -1;
    pcVar3[0xa4] = -1;
    pcVar3[0xa5] = -1;
    pcVar3[0xa6] = -1;
    pcVar3[0xa7] = -1;
    pcVar3[0xa8] = -1;
    pcVar3[0xa9] = -1;
    pcVar3[0xaa] = -1;
    pcVar3[0xab] = -1;
    *(undefined4 *)(pcVar3 + 0xac) = uVar4;
    *(undefined4 *)(pcVar3 + 0xb0) = uVar5;
    *(undefined4 *)(pcVar3 + 0xb4) = uVar6;
  }
  cVar2 = '\x01';
  pcVar3[1] = '\x01';
  if (pcVar3[0x72] == '\x02') {
    cVar2 = '\x03';
  }
  *(int *)(pcVar3 + 0xbc) = *(int *)(pcVar3 + 0xbc) + 1;
  if (pcVar3[0x72] == '\x02') {
    pcVar3[0x72] = cVar2;
  }
  return;
}


