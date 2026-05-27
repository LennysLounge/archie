    ; 2_147_483_647
    ; LDI R1, 0xFFFF
    ; LDI R2, 0x7FFF
    ; LDI R1, 21845
    ; LDI R1, 42
    LDI R1, 0

    ; LDI R1, 21845
    ; R3 BCD 0-3
    ; R4 BCD 4-7
    ; R5 BCD 8-11
    LDI R6, 0x3333
    LDI R7, 0x8888
    LDI R10, 32
    LDI R12, 1

    ; double dabble works one bit at a time from left to right.
    ; If a value has many leading zero then we are doing useless
    ; work by shifting 0s around for no reason.
    ; We accelerate this by shifting out all the zero first
    ;
    ; Remove all leading zeros: 296, 368, 496, 921
remove_leading_zeros:
    MOV R11, R2
    SHL R11, R12
    JB double
    SHL R2, R12
    SHL R1, R12
    ADDC R2, R0
    DECB R10
    JE done
    JMP remove_leading_zeros
    ; 
    ; Remove half leading zeros: 355, 356, 367, 933
    ; CMP R2, R0
    ; JNE double
    ; MOV R2, R1
    ; MOV R1, R0
    ; SHR R10, R12

    
double:
    ; DBG
    ; First we double and add one if necessary
    ; Since the working registers R1-R5 are shifted as one group
    ; we have to do a big ripple shift.
    SHL R5, R12
    SHL R4, R12
    ADDC R5, R0
    SHL R3, R12
    ADDC R4, R0
    SHL R2, R12
    ADDC R3, R0
    SHL R1, R12
    ADDC R2, R0

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

    MOV R8, R3
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
    ADD R3, R8

    ; Do the same for R4
    ; If the R4 register is still empty then we can just skip to the next round
    CMP R4, R0
    JE double
    MOV R8, R4
    ADD R8, R6
    AND R8, R7
    MOV R9, R8
    SHR R9, R12
    OR  R8, R9
    SHR R8, R12
    SHR R8, R12
    ADD R4, R8

    ; And R5
    CMP R5, R0
    JE double
    MOV R8, R5
    ADD R8, R6
    AND R8, R7
    MOV R9, R8
    SHR R9, R12
    OR  R8, R9
    SHR R8, R12
    SHR R8, R12
    ADD R5, R8

    JMP double
done:
    HALT


