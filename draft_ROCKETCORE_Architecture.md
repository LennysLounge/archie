# ROCKETCORE Virtual CPU Architecture
*Design notes for a game where players program rockets and robots*

---

## Overview

A game where players write assembly code to control rockets, robots, drones and other machines. The CPU is a configurable platform — players choose memory architecture, extensions, and peripherals based on mission requirements.

---

## The CPU Family: ROCKETCORE

### Registers

| Name | Width | Purpose |
|------|-------|---------|
| R0–R3 | 16-bit | General purpose |
| SP | 16-bit | Stack pointer |
| PC | 17-bit | Program counter (bit 16 = ROM/RAM select) |
| FLAGS | — | Zero, Carry, Overflow, Negative |
| X0, X1 | 32-bit | Extended registers (RC-2 and above) |

The FLAGS register is set automatically by arithmetic instructions. The Carry flag enables 32-bit math via register pairs on RC-1.

---

## Instruction Encoding

### 16-bit fixed-width instruction

```
[ 6-bit opcode | 2-bit Rd | 2-bit Rs | 6-bit misc ]
```

- 6-bit opcode allows 64 standard instructions (opcodes 0–62)
- Opcode `0b111111` (63) is the **escape opcode** for extended instructions

### Escape opcode — extended instruction format

When the decoder sees opcode `0b111111`, the instruction is 32 bits total:

```
Word 1: [ 111111 | 10-bit extended opcode ]   (1024 possible extended instructions)
Word 2: [ 2-bit Rd | 2-bit Rs | 12-bit misc ]
```

Extended instructions are used for FPU operations and future expansions. Old chips that don't recognise the escape opcode halt with an illegal instruction error — correct behaviour for running RC-3 code on RC-1 hardware.

### Large immediates and addresses

Instructions that need a full 16-bit immediate or jump address read the **next instruction word** as their operand, making them effectively 32-bit. This is used by `LDI` with large values and all jump instructions.

---

## Base Instruction Set (RC-1) — 24 Instructions

### Data Movement

| Instruction | Args | Description |
|-------------|------|-------------|
| MOV | Rd, Rs | Copy register to register |
| LDI | Rd, #imm | Load immediate value |
| LD | Rd, [addr] | Load from memory |
| ST | [addr], Rs | Store to memory |

### Arithmetic & Logic

| Instruction | Args | Description |
|-------------|------|-------------|
| ADD | Rd, Rs | Rd = Rd + Rs, sets flags |
| ADC | Rd, Rs | Rd = Rd + Rs + Carry (enables 32-bit math) |
| SUB | Rd, Rs | Rd = Rd - Rs, sets flags |
| SBC | Rd, Rs | Rd = Rd - Rs - Carry |
| MUL | Rd, Rs | Rd = Rd × Rs (low word) |
| AND | Rd, Rs | Bitwise AND |
| OR | Rd, Rs | Bitwise OR |
| XOR | Rd, Rs | Bitwise XOR |
| NOT | Rd | Bitwise NOT |
| SHL | Rd, #n | Shift left n bits |
| SHR | Rd, #n | Shift right n bits |

### Control Flow

| Instruction | Args | Description |
|-------------|------|-------------|
| JMP | label | Unconditional jump |
| JZ | label | Jump if Zero flag set |
| JNZ | label | Jump if Zero flag clear |
| JC | label | Jump if Carry |
| JN | label | Jump if Negative |
| CMP | Ra, Rb | Set flags from Ra - Rb, discard result |

### Stack & Subroutines

| Instruction | Args | Description |
|-------------|------|-------------|
| PUSH | Rs | Push register to stack |
| POP | Rd | Pop from stack to register |
| CALL | label | Push PC, jump to subroutine |
| RET | — | Pop PC, return from subroutine |

### System

| Instruction | Args | Description |
|-------------|------|-------------|
| HLT | — | Halt execution |

*Note: No IN/OUT instructions. All hardware access is via memory-mapped I/O (see below).*

---

## RC-2 Extensions — 32-bit Integer Math

Adds wide registers and instructions for 32-bit operations.

| Instruction | Args | Description |
|-------------|------|-------------|
| MOVX | Xd, Xs | Copy extended register |
| LDX | Xd, [addr] | Load 32-bit value from memory |
| STX | [addr], Xs | Store 32-bit value to memory |

32-bit arithmetic on RC-1 and RC-2 uses **register pairs** with ADC/SBC:

```asm
; 32-bit addition: (R0:R1) + (R2:R3) → result in R0:R1
ADD  R1, R3      ; add low words, sets carry flag
ADC  R0, R2      ; add high words plus carry
```

---

## RC-3 Extensions — Floating Point Unit

FPU instructions use the extended opcode space. All FPU operations use X0 and X1 as 32-bit float registers.

| Instruction | Cycles | Description |
|-------------|--------|-------------|
| FADD | 4–8 | X0 = X0 + X1 |
| FSUB | 4–8 | X0 = X0 - X1 |
| FMUL | 6–12 | X0 = X0 × X1 |
| FDIV | 16–32 | X0 = X0 ÷ X1 |
| FSQRT | 20–40 | X0 = √X0 |
| FSIN | 30–60 | X0 = sin(X0) |
| FCOS | 30–60 | X0 = cos(X0) |

The high cost of transcendental functions (FSIN, FCOS) makes lookup tables a compelling alternative — trading ROM space for CPU cycles.

---

## CPU Family Summary

| Chip | Features | Use Case |
|------|----------|----------|
| RC-1 | Base ISA, 16-bit registers | Simple rovers, model rockets, cheap |
| RC-2 | + 32-bit registers, MOVX/LDX/STX | Orbital guidance, long-range navigation |
| RC-3 | + FPU, transcendental functions | Autonomous guidance, trajectory correction |

All chips are **backward compatible** — RC-1 code runs unchanged on RC-3 hardware.

---

## Memory Architecture

### Address space

The PC is 17 bits wide. Bit 16 selects which physical memory is being accessed:

```
PC bit 16 = 0  →  ROM  (0x0000 – 0xFFFF, 64KB)
PC bit 16 = 1  →  RAM  (0x0000 – 0xFFFF, 64KB)
```

Normal execution lives in ROM. Jumping to an address with bit 16 set causes the CPU to fetch from RAM instead. This enables runtime code patching without a separate instruction.

### Memory map (data space)

```
0x0000 – 0xBFFF   Free RAM
0xC000 – 0xCFFF   Stack (grows downward)
0xD000 – 0xFFFF   MMIO region (device registers)
```

### Architecture modes

| Mode | Description | Tradeoff |
|------|-------------|----------|
| Harvard | Separate ROM and RAM address spaces | Full RAM available, code frozen at launch |
| Von Neumann | Code and data share one address space | Less RAM, but code can be patched mid-flight |
| Bank switching | Execute from ROM by default, jump to RAM via 17-bit PC | Best of both — full RAM normally, RAM execution as escape hatch |

The **bank switching** mode is the recommended default. Players execute from ROM for maximum RAM availability, but retain the ability to jump into RAM for patching when needed.

### Managed flash (advanced)

An optional boot ROM block handles receiving and applying code updates via radio uplink. The update process: receive new code into RAM → verify checksum → erase and rewrite application flash → reboot. The CPU stalls during flash erase/write (~1000 cycles at 1kHz = 1 full second), during which no flight control runs. Two flash partitions (A/B) allow fallback to the previous version if an update fails.

---

## Memory-Mapped I/O (MMIO)

Hardware devices appear as memory addresses. The CPU reads and writes them with normal LD and ST instructions. No special IN/OUT instructions needed.

The address decoder (part of the bus, not the CPU) routes each memory access to the appropriate device based on address range. Devices only respond to addresses in their assigned range.

### Simple devices — single address

Sensors and actuators that do one thing use a single MMIO address:

```asm
LD   R0, [0xD000]    ; read thermometer
ST   [0xD010], R1    ; write servo target angle
```

The device continuously updates its address (for sensors) or reads it (for actuators). The CPU just reads or writes normally.

### Complex devices — Control/Status Register (CSR) interface

Devices with multiple operations expose a cluster of addresses:

| Register | Name | Direction | Description |
|----------|------|-----------|-------------|
| 0xF0 | CR (Control Register) | Write | Which operation to perform |
| 0xF1 | DR (Data Register) | Read/Write | Argument or result |
| 0xF2 | SR (Status Register) | Read | Busy flag, error flag, done flag |

The CPU writes a command code to CR, writes arguments to DR, polls SR until the busy bit clears, then reads results from DR. Example with a radio:

```asm
; Wait until radio is ready
WAIT:
    LD   R0, [0xF2]     ; read status register
    LDI  R1, #1
    AND  R0, R1
    JNZ  WAIT           ; bit 0 = busy, keep waiting

; Transmit a byte
    LDI  R0, #0x01      ; command: transmit
    ST   [0xF0], R0
    LDI  R0, #0x42      ; data to send
    ST   [0xF1], R0
```

### Device synchronisation

**Handshake (simple):** Device sets a "data ready" bit in SR. CPU reads data, then clears the bit to acknowledge. Device waits for acknowledgement before writing the next value. Simple but CPU is stuck polling.

**Hardware buffer (complex devices):** Device has internal memory. From the CPU's perspective it still looks like DR + SR — the SR bit indicates "more data waiting" until the buffer drains. The CPU never manages pointers. Buffer overflow behaviour (drop data, overwrite, set error flag) is defined per device.

**Interrupts (advanced):** Device signals the CPU when data is ready. CPU pauses main loop, jumps to interrupt handler, reads data, resumes. Requires interrupt vector table and RETI instruction. Eliminates polling loops entirely.

### Device internal memory

Devices have their own internal buffers not accessible by the CPU directly. The CPU only touches MMIO interface registers. There is no bus conflict because device memory and CPU RAM are physically separate:

```
[ CPU ] ←——bus——→ [ RAM ]

[ CPU ] ←— MMIO registers —→ [ Device | internal buffer ]
```

---

## Example Device Interfaces

### Rocket engine

| Address | Name | Direction | Description |
|---------|------|-----------|-------------|
| 0xD000 | THRUST | Write | Engine thrust 0–255 |
| 0xD001 | GIMBAL | Write | Nozzle angle -128–127 |
| 0xD002 | ALTITUDE | Read | Current altitude |
| 0xD003 | VELOCITY | Read | Current velocity |
| 0xD004 | FUEL | Read | Remaining fuel |
| 0xD005 | ARMED | Write | Write 1 to ignite |

### Robot drive

| Address | Name | Direction | Description |
|---------|------|-----------|-------------|
| 0xD000 | MOTOR_L | Write | Left motor speed |
| 0xD001 | MOTOR_R | Write | Right motor speed |
| 0xD002 | SENSOR_F | Read | Front distance |
| 0xD003 | SENSOR_L | Read | Left distance |
| 0xD004 | GYRO | Read | Rotation angle |

### Radio transceiver (CSR interface)

| Address | Name | Direction | Description |
|---------|------|-----------|-------------|
| 0xF0 | RADIO_CR | Write | 0x01=transmit, 0x02=set channel, 0x03=receive |
| 0xF1 | RADIO_DR | Read/Write | Byte to send or received byte |
| 0xF2 | RADIO_SR | Read | Bit 0=busy, bit 1=message waiting, bit 2=error |

---

## Timing Model

Base clock: **1kHz** (1000 cycles per second, 1ms per cycle)

### Instruction costs

| Operation | Cycles |
|-----------|--------|
| MOV, ADD, SUB, logic | 1 |
| LD, ST (RAM) | 2 |
| LD, ST (MMIO) | 3–5 |
| MUL | 4–8 |
| DIV | 16–32 |
| FADD, FSUB | 4–8 |
| FMUL | 6–12 |
| FDIV | 16–32 |
| FSQRT | 20–40 |
| FSIN, FCOS | 30–60 |
| Flash erase/write | ~1000 |
| Fetch from ROM | 2 cycles/instruction |
| Fetch from RAM | 1 cycle/instruction |

At 1kHz, a basic PID control loop (read sensors, compute error, write output) takes roughly 30–50 cycles, giving a control update rate of 20–30Hz. Adding radio handling, sensor fusion, and fault detection makes the CPU budget feel genuinely tight.

### Optimisation opportunities players discover naturally

- Replace FSIN/FCOS with lookup tables in ROM (cycles → ROM space tradeoff)
- Small angle approximation: sin(x) ≈ x for small x, no trig needed
- Run expensive calculations less frequently than simple ones
- Upgrade clock speed (buy a faster crystal oscillator)
- Multiply-by-reciprocal instead of divide for constant divisors

---

## Upgrade System

Every upgrade has costs beyond money: mass, power draw, volume.

| Upgrade | Benefit | Cost |
|---------|---------|------|
| More ROM | Larger programs, bigger lookup tables | Mass, power |
| More RAM | More runtime data, larger buffers | Mass, power |
| RC-2 32-bit extension | 32-bit integer math, 4GB address range | Slightly larger chip |
| RC-3 FPU | Hardware floating point, transcendentals | Expensive, heavy, power-hungry |
| Faster clock | More instructions per second | Power, heat |
| Harvard mode | Full RAM available for data | Code frozen at launch |
| Bank switching | Full RAM + optional RAM execution | Requires wider PC |
| Managed flash | Mid-flight code updates | Requires radio peripheral |
| Radio peripheral | Telemetry, patching, swarm coordination | Mass, power, antenna |

### Example configurations

**Atmospheric probe** — RC-1, 2KB ROM, 512B RAM, Von Neumann, simple sensor polling, scaled integer math for pressure and temperature.

**Orbital guidance computer** — RC-3 with FPU, 32KB ROM (guidance algorithms + lookup tables), 8KB RAM, Harvard architecture, full 32-bit position math.

**Swarm drone** — RC-1 (mass critical), 4KB ROM, 1KB RAM, radio peripheral for coordination, lookup tables for trig instead of FPU.

---

## Example Program — Rocket Hover Controller

```asm
; Hover at altitude 100
; Reads altitude sensor, adjusts thrust

START:
    LD   R0, [0xD002]   ; read altitude
    LDI  R1, #100       ; target altitude
    CMP  R0, R1         ; altitude - target
    JZ   HOLD           ; exactly on target
    JN   TOO_LOW        ; negative = below target

TOO_HIGH:
    LDI  R2, #80        ; reduce thrust
    ST   [0xD000], R2
    JMP  START

TOO_LOW:
    LDI  R2, #180       ; increase thrust
    ST   [0xD000], R2
    JMP  START

HOLD:
    LDI  R2, #128       ; neutral thrust
    ST   [0xD000], R2
    JMP  START
```

---

## Design Philosophy

**The constraints are the game.** 1kHz clock, limited RAM, no FPU on RC-1 — these aren't artificial limitations, they're the same pressures real embedded engineers face. Players who can't afford the FPU discover fixed-point math. Players tight on RAM write memory-efficient code. The architecture choice (Harvard vs Von Neumann) stops being a technical decision and becomes a statement about how much the player trusts their own code.

**The code you upload is the code that flies.** With Harvard architecture there is no hotfix, no remote debug, no patch. You have to get it right before launch. This creates genuine tension that no scripted game event can match.

**The ISA is stable.** The escape opcode gives 1024 extended instructions. MMIO means new hardware never requires CPU changes. Backward compatibility means RC-1 code runs unchanged on RC-3. Any future additions slot in without redesigning the architecture.
