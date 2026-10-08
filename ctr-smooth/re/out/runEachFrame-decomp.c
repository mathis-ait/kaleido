// ===== FUN_0010e354 @ 0010e354 (1028 octets)

undefined4 FUN_0010e354(int param_1)

{
  longlong lVar1;
  ulonglong uVar2;
  longlong lVar3;
  bool bVar4;
  byte bVar5;
  int iVar6;
  int *piVar7;
  uint uVar8;
  uint uVar9;
  int iVar10;
  uint uVar11;
  char unaff_r6;
  undefined4 uVar12;
  uint uVar13;
  int iVar14;
  undefined8 uVar15;
  undefined4 local_38;
  
  uVar12 = DAT_0010e690;
  local_38 = 1;
  if (*(char *)(param_1 + 0xd) == '\0') {
LAB_0010e394:
    unaff_r6 = '\x01';
    if (**(int **)(param_1 + 0x18) == 0) {
      iVar10 = *(int *)(param_1 + 0x14);
      iVar6 = FUN_001233b4(iVar10);
      if (iVar6 == 2) {
        piVar7 = (int *)FUN_001233c4(iVar10);
        (**(code **)(*piVar7 + 0x20))(piVar7,*(undefined4 *)(iVar10 + 0x14));
      }
    }
    FUN_00117ecc(*(undefined4 *)(param_1 + 0x18));
    FUN_00122c6c(*(undefined4 *)(param_1 + 0x2c));
    FUN_0012ddf4(*(undefined4 *)(param_1 + 0x30));
    iVar6 = *(int *)(param_1 + 0x14);
    if (iVar6 != 0) {
      *(undefined4 *)(iVar6 + 0x18) = 0;
      iVar10 = FUN_001233b4(iVar6);
      if (iVar10 == 2) {
        *(undefined4 *)(iVar6 + 0x18) = 1;
      }
      iVar10 = FUN_0011ed7c(iVar6);
      if ((iVar10 == 2 || iVar10 == 3) &&
         (piVar7 = (int *)FUN_001233c4(iVar6), piVar7 != (int *)0x0)) {
        iVar14 = *(int *)(iVar6 + 0x14);
        uVar8 = (**(code **)(*piVar7 + 0x18))();
        if (uVar8 < 2) {
          *(char *)(iVar14 + 0xc) = (char)uVar8;
        }
        *(undefined4 *)(*(int *)(iVar6 + 0x14) + 0x10) = uVar12;
      }
      if (iVar10 == 0) {
        local_38 = 0;
        goto LAB_0010e530;
      }
    }
    local_38 = 1;
  }
  else {
    bVar5 = *(byte *)(param_1 + 0xe) ^ 1;
    if (bVar5 == 0) {
      unaff_r6 = '\0';
    }
    *(byte *)(param_1 + 0xe) = bVar5;
    if (bVar5 != 0) goto LAB_0010e394;
    iVar6 = *(int *)(param_1 + 0x14);
    if (((iVar6 != 0) && (*(int *)(iVar6 + 0x18) != 0)) &&
       (iVar10 = FUN_001233b4(iVar6), iVar10 == 2)) {
      piVar7 = (int *)FUN_001233c4(iVar6);
      iVar10 = (**(code **)(*piVar7 + 0x1c))(piVar7,iVar6);
      if (iVar10 == 1) {
        FUN_0011ecd4(iVar6,3);
        piVar7[1] = 0;
      }
    }
  }
LAB_0010e530:
  if ((*(byte *)(param_1 + 0xd) & -unaff_r6) != 0) {
    return local_38;
  }
  uVar8 = *(uint *)(param_1 + 0x1c);
  if (*(char *)(uVar8 + 0x1fe) != '\0') {
    *(undefined1 *)(uVar8 + 0x1fe) = 0;
    return local_38;
  }
  software_interrupt(0x28);
  uVar15 = 0;
  if (*(int *)(param_1 + 0x14) != 0) {
    uVar12 = *(undefined4 *)(DAT_0010e694 + 4);
    FUN_001117a4(uVar12,7);
    if (*(char *)(*(int *)(param_1 + 0x14) + 10) != '\0') {
      FUN_0011ece4();
    }
    FUN_0011cb60();
    FUN_00122be0();
    FUN_0012286c();
    FUN_00122be0();
    FUN_00122810();
    FUN_00142350(uVar12,1);
    FUN_0011ce44(uVar12);
    uVar15 = FUN_0011153c(uVar12);
  }
  software_interrupt(0x28);
  uVar13 = (uint)uVar15 - uVar8;
  uVar11 = (int)((ulonglong)uVar15 >> 0x20) - (uint)((uint)uVar15 < uVar8);
  bVar4 = false;
  uVar8 = DAT_0010e69c;
  if (*(char *)(param_1 + 0xd) != '\0') {
    uVar8 = DAT_0010e6a0;
  }
  if ((*(char *)(*(int *)(param_1 + 0x1c) + 0x1fc) == '\x01') &&
     (uVar9 = FUN_0011d62c(), uVar8 < uVar9)) {
    bVar4 = true;
  }
  if (*(char *)(*(int *)(param_1 + 0x1c) + 0x1fd) == '\x01') {
    uVar2 = (ulonglong)uVar13 * 3 +
            ((ulonglong)
             (((int)uVar11 >> 0x1f) * DAT_0010e76c +
             (int)((ulonglong)DAT_0010e76c * (ulonglong)uVar11 >> 0x20)) << 0x20 |
            (ulonglong)DAT_0010e76c * (ulonglong)uVar11 & 0xffffffff) +
            CONCAT44(uVar11 * 3,(int)((ulonglong)DAT_0010e76c * (ulonglong)uVar13 >> 0x20));
    uVar11 = (uint)(uVar2 >> 0x20);
    iVar6 = (int)uVar11 >> 0x1f;
    lVar3 = (ulonglong)DAT_0010e770 * (ulonglong)uVar11 +
            CONCAT44(iVar6 * DAT_0010e770,
                     (int)((ulonglong)DAT_0010e770 * (uVar2 & 0xffffffff) >> 0x20));
    lVar1 = (ulonglong)DAT_0010e774 * (uVar2 & 0xffffffff);
    uVar9 = (uint)((ulonglong)lVar3 >> 0x20);
    uVar13 = (uint)((ulonglong)DAT_0010e774 * (ulonglong)uVar11);
    uVar8 = (int)((ulonglong)lVar1 >> 0x20) + (uint)CARRY4((uint)lVar3,(uint)lVar1);
    if (DAT_0010e69c <
        (uVar8 + uVar9 + uVar13 >> 7 |
        (((int)uVar8 >> 0x1f) + ((int)uVar9 >> 0x1f) + (uint)CARRY4(uVar8,uVar9) +
         iVar6 * DAT_0010e774 + (int)((ulonglong)DAT_0010e774 * (ulonglong)uVar11 >> 0x20) +
        (uint)CARRY4(uVar8 + uVar9,uVar13)) * 0x2000000) - iVar6) goto LAB_0010e680;
  }
  if (!bVar4) {
    return local_38;
  }
LAB_0010e680:
  *(undefined1 *)(*(int *)(param_1 + 0x1c) + 0x1fe) = 1;
  return local_38;
}


// ===== FUN_00108a48 @ 00108a48 (212 octets)

int * FUN_00108a48(int param_1)

{
  char cVar1;
  int iVar2;
  int *piVar3;
  undefined4 uVar4;
  
  cVar1 = *(char *)(param_1 + 0xc);
  if (cVar1 != '\x02') {
    if (*(char *)(param_1 + 0xd) != cVar1) {
      uVar4 = *DAT_00108b1c;
      if (cVar1 == '\0') {
        FUN_00110f14(uVar4,0);
      }
      else {
        FUN_00110f14(uVar4,1);
        FUN_00111008(uVar4);
      }
      *(undefined1 *)(param_1 + 0xe) = 0;
      *(undefined1 *)(param_1 + 0xd) = *(undefined1 *)(param_1 + 0xc);
    }
    *(undefined1 *)(param_1 + 0xc) = 2;
  }
  FUN_00113784(*(undefined4 *)(param_1 + 0x24));
  FUN_00114598(*(undefined4 *)(param_1 + 0x40));
  FUN_0010e778(*(undefined4 *)(param_1 + 0x20),*(undefined4 *)(*(int *)(param_1 + 0x1c) + 0x18));
  iVar2 = FUN_0010e354(param_1);
  if (iVar2 == 0) {
    piVar3 = *(int **)(param_1 + 0x18);
    iVar2 = *piVar3;
    if (iVar2 == 0) {
      piVar3 = (int *)(int)(char)piVar3[3];
    }
    if (iVar2 == 0 && piVar3 == (int *)0x0) {
      return piVar3;
    }
  }
  return (int *)&DAT_00000001;
}


