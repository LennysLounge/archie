; -------------------------------------------------------
; Fictional Processor Program: Multiply R0 by R1
; Result is stored in R2
; -------------------------------------------------------

main:
    LDI R0, 5           ; Load immediate value 5 into R0 (Multiplicand)
    LDI R1, 3           ; Load immediate value 3 into R1 (Multiplier)
    LDI R2, 0           ; Initialize result (R2) to 0
    
loop:
    CMP R1, 0           ; Compare Multiplier to 0
    JE  end_prog        ; If R1 == 0, we are done, jump to end
    
    ADD R2, R0          ; R2 = R2 + R0
    DECB R1             ; Decrement R1 by 1
    JMP loop            ; Repeat the loop

end_prog:
    PUSH R2             ; Save result to stack
    CALL print_val      ; Call a fictional print subroutine
    HALT                ; Stop execution

; --- Subroutine: print_val ---
print_val:
    ; (Imagine code here to interface with a peripheral)
    RET                 ; Return to caller