    LDI R1, 21845
    ;LDI R1, 42
    LDI R12, 1
    LDI R11, 4
    LDI R9, 16
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
    LDI R4, 3
    LDI R5, 5
    LDI R6, 0b1111
    LDI R8, 4
dabble:
    MOV R7, R2
    AND R7, R6
    CMP R7, R5
    JL was_less_than_5
    ADD R2, R4
was_less_than_5:
    SHL R4, R11
    SHL R5, R11
    SHL R6, R11
    DECB R8
    JNE dabble
    JMP double
done:
    HALT
