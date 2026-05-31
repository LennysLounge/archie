    ; 2_147_483_647
    ; LDI R1, 0xFFFF
    ; LDI R2, 0x7FFF
    LDI R1, 21845
    ; LDI R1, 42
    ;LDI R1, 0
    CALL bin32_to_bcd
    
    PUSH R1
    PUSH R2
    PUSH R3

    LDI R1, 3
    LDI R4, 4
    LDI R6, 48
    LDI R7, 0 ; 0 if we are skipping leading zeros, 1 if we are printing zeros
print_word:
    LDI R2, 0xF000
    LDI R3, 12
print_nibble:
    LDW R5, [SP]
    AND R5, R2
    SHR R5, R3
    JNE print_char
        CMP R7, R0
        JNE print_char
        JMP skip_char
print_char:
    ADD R5, R6
    STB [R0 + 0xFFFF], R5
    LDI R7, 1
skip_char:
    SUB R3, R4
    SHR R2, R4
    JNE print_nibble
    POP R0
    DECB R1
    JNE print_word

    LDI R1, 10
    STB [R0+0xFFFF], R1
    LDI R1, 13
    STB [R0+0xFFFF], R1


    HALT
    NOP
    NOP


; Converts an unsigned 32 bit value into its packed bcd representation
; Parameters: 
;   R1      low word of 32 bit value (bits 0-15)
;   R2      high word of 32 bit value (bits 16-31)
; Return:
;   R1      packed bcd digits 0-3
;   R2      packed bcd digits 4-7
;   R3      packed bcd digits 8-9   (high byte is zeroed)
bin32_to_bcd:
    ; Save registers
    PUSH R5
    PUSH R6
    PUSH R7
    PUSH R8
    PUSH R9
    PUSH R10
    PUSH R12

    ; Body
    ; Move the input away from R1,R2 to make room for the result
    ; and at the same time check if the input is zero
;     MOV R4, R1
;     JNE input_is_not_zero_1
;     MOV R5, R2
;     JNE input_is_not_zero_2
;         LDI R1, 0
;         LDI R2, 0
;         LDI R3, 0
;         JMP done
; input_is_not_zero_1:
;     MOV R5, R2
; input_is_not_zero_2:

    ; Body
    ; Move the input away from R1,R2 to make room for the result
    MOV R4, R1
    MOV R5, R2
    LDI R1, 0
    LDI R2, 0
    LDI R3, 0
    ; Load some constants that are needed
    LDI R6, 0x3333
    LDI R7, 0x8888
    LDI R10, 32
    LDI R12, 1

    ; For a small performance optimization we skip half of the iterations
    ; if the high input word is empty.
    CMP R5, R0
    JNE skip
    MOV R5, R4
    SHR R10, R12
skip:

    ; Get into the double dabble algorithm
double:
    ; DBG
    ; First we double and add one if necessary
    ; Since the working registers R1-R5 are shifted as one group
    ; we have to do a big ripple shift.
    SHL R3, R12
    SHL R2, R12
    ADDC R3, R0
    SHL R1, R12
    ADDC R2, R0
    SHL R5, R12
    ADDC R1, R0
    SHL R4, R12
    ADDC R5, R0

    DECB R10
    JE done

    ; Now we need to fix up any rounding issues by doing the dabble part
    ; Any nibble that is 5 or higher gets 3 added.
    ; This will cause a correct carry the next time everything is doubled.
    ; This works because double of 5 and 3 is 10 and 6. You have to add 6
    ; to a 10 to get it to round in bas 16.

    ; We can do this for all four nibbles at the same time.
    ; First we have to figure out which nibbles need 3 added. We can do this
    ; by adding 3 to every nibble and seeing which ones have the third bit set.
    ; This works because only values >=5 will have the third bit set after adding 3.

    MOV R8, R1
    JE mid
    ADD R8, R6
    AND R8, R7

    ; Now we have a mask that has a bit set for ever nibble that needs 3 added 
    ; For example the original value 0x5273 will look like this 0x8080 or 0b 1000_0000_1000_0000
    MOV R9, R8
    SHR R9, R12
    OR  R8, R9
    SHR R8, R12
    SHR R8, R12

    ; Now R8 contains only three for the nibbles that need to have three added to them
    ADD R1, R8

mid:
    ; If the R4 register is still empty then we can just skip to the next round
    MOV R8, R2
    JE high
    ADD R8, R6
    AND R8, R7
    MOV R9, R8
    SHR R9, R12
    OR  R8, R9
    SHR R8, R12
    SHR R8, R12
    ADD R2, R8

high:
    MOV R8, R3
    JE double
    ADD R8, R6
    AND R8, R7
    MOV R9, R8
    SHR R9, R12
    OR  R8, R9
    SHR R8, R12
    SHR R8, R12
    ADD R3, R8

    JMP double
done:
    ; Epilogue
    POP R12
    POP R10
    POP R9
    POP R8
    POP R7
    POP R6
    POP R5

    RET


