    LDI R1, 65
start:
    STB [R0 + 0xFFFF], R1
    JMP start