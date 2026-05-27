    LDI R1, 21845
    ;LDI R1, 42
    LDI R12, 1
    LDI R11, 4
    LDI R9, 16
    LDI R4, 0x3333
    LDI R5, 0x8888
    ;DBG
double:
    SHL R3, R12
    SHL R2, R12
    ADDC R3, R0
    SHL R1, R12
    ADDC R2, R0
    ;DBG
    DECB R9
    JE done
    ; dabble

    MOV R7, R2
    ADD R7, R4
    AND R7, R5
    MOV R8, R7
    SHR R8, R12
    OR  R7, R8
    SHR R7, R12
    SHR R7, R12
    ADD R2, R7

    JMP double
done:
    HALT
