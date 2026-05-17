    LDI R1, 1
    LDI R2, 3
    LDI R3, 5
    LDI R7, 1000
loop:
    DECB R2
    JNE skip_3
    MOV R4, R1
    LDI R2, 3
skip_3:
    DECB R3
    JNE skip_5
    MOV R4, R1
    LDI R3, 05
skip_5:
    ADD R5, R4
    ADDC R6, R0
    MOV R4, R0
    INCB R1
    CMP R1, R7
    JL loop

    ; // let program = vec![
    ; //     // setup variables
    ; //     LDI_POS(R1, u4::new(1)),
    ; //     LDI_POS(R2, u4::new(3)),
    ; //     LDI_POS(R3, u4::new(5)),
    ; //     LDI_IMM(R7, 1000),
    ; //     //
    ; //     DECB(R2),
    ; //     JNE(2),
    ; //     MOV(R4, R1),
    ; //     LDI_POS(R2, u4::new(3)),
    ; //     DECB(R3),
    ; //     JNE(2),
    ; //     MOV(R4, R1),
    ; //     LDI_POS(R3, u4::new(5)),
    ; //     ADD(R5, R4),
    ; //     ADDC(R6, R0),
    ; //     MOV(R4, R0),
    ; //     INCB(R1),
    ; //     CMP(R1, R7),
    ; //     JL(-14),
    ; // ]; // 11059

    ; // let program = vec![
    ; //     // setup variables
    ; //     LDI_POS(R1, u4::new(1)),
    ; //     LDI_POS(R2, u4::new(3)),
    ; //     LDI_POS(R3, u4::new(5)),
    ; //     LDI_IMM(R7, 1000),
    ; //     //
    ; //     DECB(R2),
    ; //     JNE(7),
    ; //     ADD(R5, R1),
    ; //     ADDC(R6, R0),
    ; //     LDI_POS(R2, u4::new(3)),
    ; //     DECB(R3),
    ; //     JNE(1),
    ; //     LDI_POS(R3, u4::new(5)),
    ; //     JMP_OFF(5),
    ; //     DECB(R3),
    ; //     JNE(3),
    ; //     LDI_POS(R3, u4::new(5)),
    ; //     ADD(R5, R1),
    ; //     ADDC(R6, R0),
    ; //     INCB(R1),
    ; //     CMP(R1, R7),
    ; //     JL(-17),
    ; // ]; // 8795

    ; // let program = vec![
    ; //     LDI_POS(R1, u4::new(0)),  // i
    ; //     LDI_POS(R2, u4::new(15)), // const
    ; //     LDI_IMM(R3, 1000),        // const
    ; //     // R5 acc1
    ; //     // R6 acc2
    ; //     LDI_IMM(R8, 60),
    ; //     MOV(R4, R1),
    ; //     //
    ; //     ADD(R4, R2),
    ; //     CMP(R4, R3),
    ; //     JGE(13),
    ; //     MOV(R7, R0),
    ; //     ADD(R7, R1),
    ; //     ADD(R7, R1),
    ; //     ADD(R7, R1),
    ; //     ADD(R7, R1),
    ; //     ADD(R7, R1),
    ; //     ADD(R7, R1),
    ; //     ADD(R7, R1),
    ; //     ADD(R7, R8),
    ; //     ADD(R5, R7),
    ; //     ADDC(R6, R0),
    ; //     MOV(R1, R4),
    ; //     JMP_OFF(-16),
    ; //     // DBG,
    ; //     INCB(R1),
    ; //     LDI_POS(R2, u4::new(3)),
    ; //     LDI_POS(R4, u4::new(5)),
    ; //     DECB(R2),
    ; //     JNE(7),
    ; //     ADD(R5, R1),
    ; //     ADDC(R6, R0),
    ; //     LDI_POS(R2, u4::new(3)),
    ; //     DECB(R4),
    ; //     JNE(1),
    ; //     LDI_POS(R4, u4::new(5)),
    ; //     JMP_OFF(5),
    ; //     DECB(R4),
    ; //     JNE(3),
    ; //     LDI_POS(R4, u4::new(5)),
    ; //     ADD(R5, R1),
    ; //     ADDC(R6, R0),
    ; //     INCB(R1),
    ; //     CMP(R1, R3),
    ; //     JL(-17),
    ; // ]; // 1147