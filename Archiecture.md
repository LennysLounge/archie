Design notes for a fictional cpu architecture to program rockets and robots.

# ISA Reference

## Registers

| Address | Name | Description |
|---------|------|-------------|
| `0x0` | `R0` | Constant zero — always reads 0, writes do nothing |
| `0x1–0xC` | `R1–R12` | General purpose registers |
| `0xD` | `ST` | Status register — contains flags |
| `0xE` | `SP` | Stack pointer — points to RAM |
| `0xF` | `PC` | Program counter — points to ROM |

All registers are 16-bit wide.


## Flags

Flags reflect the status of the last instruction.

| Flag | Name | Description |
|------|------|-------------|
| `ZF` | Zero | Set on any write to a register based on the written value |
| `NF` | Negative | Set on any write to a register based on the written value |
| `CF` | Carry | Set if the result lost bits beyond the 16-bit boundary. For right shifts, set if any 1-bit was shifted out of bit 0 |
| `OF` | Overflow | Set if the sign of the result changed unexpectedly. See rules below |
| `IF` | Interrupt Disabled | Set only by explicit enable/disable instructions |

### Overflow Rules

| Context | Rule |
|---------|------|
| Two-operand arithmetic (`ADD`, `SUB`, `CMP`) | `OF = 1` if `sign(Ra) == sign(Rb)` and `sign(Ra) != sign(Rres)` |
| Single-operand arithmetic (shifts) | `OF = 1` if `sign(Ra) != sign(Rres)` |
| Bitwise logic (`AND`, `OR`, `XOR`, `NOT`) | `OF = 0` always |

### Flag Notation

| Symbol | Meaning |
|--------|---------|
| `#` | Set based on the result of the instruction |
| `0` | Always cleared |
| `1` | Always set |
| `-` | Unchanged |


## Addressing Modes

Applies to memory access instructions. The mode occupies 2 bits in the instruction encoding.

| Mode | Syntax | Description |
|------|--------|-------------|
| `00` | `[Rn]` | Address is the value in `Rn` |
| `01` | `[Rn++]` | Address is the value in `Rn`, then `Rn` increments by 1 (byte) or 2 (word) |
| `10` | `[--Rn]` | `Rn` decrements by 1 (byte) or 2 (word), then address is the new value in `Rn` |
| `11` | `[Rn + imm16]` | Address is `Rn` plus a signed offset in the following immediate word |

Absolute addressing is achieved with `[R0 + imm16]` since `R0` is always zero.


## Instruction Fields

| Field | Description |
|-------|-------------|
| `Ra` | First register operand — also the destination for in-place operations |
| `Rb` | Second register operand |
| `Rn` | Register used as a memory address (in mode expressions) |
| `imm16` | 16-bit immediate value stored in the word following the instruction |
| `imm4` | 4-bit immediate value encoded directly in the instruction word |
| `offset` | Signed word offset for branch instructions, encoded in the instruction word |
| `ivec` | Interrupt vector number |
| `BW` | Selects byte (`0`) or word (`1`) operation |
| `S` | Sign-extension bit — extends byte reads with sign (`1`) or zero (`0`) |


## Instruction Set

### System

| Instruction | Encoding | `ZF` | `NF` | `CF` | `OF` | `IF` |
|-------------|----------|------|------|------|------|------|
| No operation | `0000 0000 0000 0000` | `-` | `-` | `-` | `-` | `-` |
| Halt — interrupts still trigger | `0000 1111 1111 1111` | `-` | `-` | `-` | `-` | `-` |
| Disable maskable interrupts | `0000 0000 0110 ----` | `-` | `-` | `-` | `-` | `1` |
| Enable maskable interrupts | `0000 0000 0111 ----` | `-` | `-` | `-` | `-` | `0` |
| Trigger interrupt `ivec` | `0000 0000 0100 ivec` | `-` | `-` | `-` | `-` | `-` |
| Return from interrupt | `0000 0000 0101 ----` | `-` | `-` | `-` | `-` | `-` |

### Subroutines

| Instruction | Encoding | `ZF` | `NF` | `CF` | `OF` | `IF` |
|-------------|----------|------|------|------|------|------|
| Call subroutine at `imm16` | `0000 0000 0001 ----` + `imm16` | `-` | `-` | `-` | `-` | `-` |
| Call subroutine at address in `Ra` | `0000 0000 0010 Ra--` | `-` | `-` | `-` | `-` | `-` |
| Return from subroutine | `0000 0000 0011 ----` | `-` | `-` | `-` | `-` | `-` |

### Memory

| Instruction | Encoding | `ZF` | `NF` | `CF` | `OF` | `IF` |
|-------------|----------|------|------|------|------|------|
| Load from RAM `mode(Rn)` into `Ra` | `0001 S BW mm Rn-- Ra--` | `#` | `#` | `-` | `-` | `-` |
| Load from ROM `mode(Rn)` into `Ra` | `0010 S BW mm Rn-- Ra--` | `#` | `#` | `-` | `-` | `-` |
| Store `Ra` into RAM `mode(Rn)` | `0011 0 BW mm Ra-- Rn--` | `-` | `-` | `-` | `-` | `-` |

### Register Operations

| Instruction | Encoding | `ZF` | `NF` | `CF` | `OF` | `IF` |
|-------------|----------|------|------|------|------|------|
| Load `imm16` into `Ra` | `0100 0000 0000 Ra--` + `imm16` | `#` | `#` | `-` | `-` | `-` |
| Load `imm4` into `Ra`, zero-pad | `0100 0000 0001 Ra-- imm4` | `#` | `#` | `-` | `-` | `-` |
| Load `imm4` into `Ra`, one-pad | `0100 0000 0010 Ra-- imm4` | `#` | `#` | `-` | `-` | `-` |
| Move `Ra` into `Rb` | `0100 0000 0011 Ra-- Rb--` | `#` | `#` | `-` | `-` | `-` |
| Push `Ra` onto stack | `0100 1110 Ra-- ----` | `-` | `-` | `-` | `-` | `-` |
| Pop stack into `Ra` | `0100 1111 Ra-- ----` | `#` | `#` | `-` | `-` | `-` |

### Jumps and Branches

All branch offsets are signed and measured in words from the current PC.

| Instruction | Condition | Encoding | `ZF` | `NF` | `CF` | `OF` | `IF` |
|-------------|-----------|----------|------|------|------|------|------|
| Jump to `imm16` | always | `0101 0000 0000 ----` + `imm16` | `-` | `-` | `-` | `-` | `-` |
| Jump to address in `Ra` | always | `0101 0001 Ra-- ----` | `-` | `-` | `-` | `-` | `-` |
| Jump by `offset` | always | `0101 0010 offset---` | `-` | `-` | `-` | `-` | `-` |
| Jump if equal | `ZF = 1` | `0101 0011 offset---` | `-` | `-` | `-` | `-` | `-` |
| Jump if not equal | `ZF = 0` | `0101 0100 offset---` | `-` | `-` | `-` | `-` | `-` |
| Jump if less | `NF ≠ OF` | `0101 0101 offset---` | `-` | `-` | `-` | `-` | `-` |
| Jump if less or equal | `NF ≠ OF or ZF = 1` | `0101 0110 offset---` | `-` | `-` | `-` | `-` | `-` |
| Jump if greater | `NF = OF and ZF = 0` | `0101 0111 offset---` | `-` | `-` | `-` | `-` | `-` |
| Jump if greater or equal | `NF = OF` | `0101 1000 offset---` | `-` | `-` | `-` | `-` | `-` |
| Jump if below | `CF = 1` | `0101 1001 offset---` | `-` | `-` | `-` | `-` | `-` |
| Jump if below or equal | `CF = 1 or ZF = 1` | `0101 1010 offset---` | `-` | `-` | `-` | `-` | `-` |
| Jump if above | `CF = 0 and ZF = 0` | `0101 1011 offset---` | `-` | `-` | `-` | `-` | `-` |
| Jump if above or equal | `CF = 0` | `0101 1100 offset---` | `-` | `-` | `-` | `-` | `-` |

### ALU

| Instruction | Encoding | `ZF` | `NF` | `CF` | `OF` | `IF` |
|-------------|----------|------|------|------|------|------|
| `Ra = Ra + Rb` | `0110 0000 Ra-- Rb--` | `#` | `#` | `#` | `#` | `-` |
| `Ra = Ra + Rb + CF` | `0110 0001 Ra-- Rb--` | `#` | `#` | `#` | `#` | `-` |
| `Ra = Ra - Rb` | `0110 0010 Ra-- Rb--` | `#` | `#` | `#` | `#` | `-` |
| `Ra = Ra - Rb - CF` | `0110 0011 Ra-- Rb--` | `#` | `#` | `#` | `#` | `-` |
| `Ra = Ra AND Rb` | `0110 0100 Ra-- Rb--` | `#` | `#` | `0` | `0` | `-` |
| `Ra = Ra OR Rb` | `0110 0101 Ra-- Rb--` | `#` | `#` | `0` | `0` | `-` |
| `Ra = Ra XOR Rb` | `0110 0110 Ra-- Rb--` | `#` | `#` | `0` | `0` | `-` |
| `Ra = NOT Ra` | `0110 0111 Ra-- ----` | `#` | `#` | `0` | `0` | `-` |
| `Ra = Ra << Rb` | `0110 1000 Ra-- Rb--` | `#` | `#` | `#` | `#` | `-` |
| `Ra = Ra >> Rb` (logical) | `0110 1001 Ra-- Rb--` | `#` | `0` | `#` | `#` | `-` |
| `Ra = Ra >>> Rb` (arithmetic) | `0110 1010 Ra-- Rb--` | `#` | `#` | `#` | `0` | `-` |
| Compare `Ra` to `Rb` | `0110 1111 Ra-- Rb--` | `#` | `#` | `#` | `#` | `-` |


    call    r0
    load    r0
    load u8 r0
    load i8 r0

    LDW     r0
    LDB     r0
    LDS     r0
     