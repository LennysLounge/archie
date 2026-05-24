use std::io::{self, Stdout};

use crossterm::{cursor::MoveTo, queue, style::Print};

#[allow(non_camel_case_types)]
enum InFlightOp {
    LDI_IMM {
        dst: usize,
    },
    JMP_IMM,
    LD_IMM {
        signed: bool,
        byte_mode: bool,
        dst: usize,
        base_addr: u16,
    },
    LR_IMM {
        signed: bool,
        byte_mode: bool,
        dst: usize,
        base_addr: u16,
    },
    ST_IMM {
        byte_mode: bool,
        base_addr: u16,
        value: u16,
    },
}
pub struct MCU {
    pub register: [u16; 16],
    pub ram: [u8; u16::MAX as usize + 1],
    pub rom: Vec<u16>,
    pub cycle_counter: u32,
    in_flight_op: Option<InFlightOp>,
    pub last_inst: Option<u16>,
}
impl MCU {
    pub fn new(program: Vec<u16>) -> Self {
        Self {
            register: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xF000, 0],
            ram: [0; u16::MAX as usize + 1],
            rom: program,
            cycle_counter: 0,
            in_flight_op: None,
            last_inst: None,
        }
    }

    fn read_rom_and_advance_pc(&mut self) -> Option<u16> {
        let inst = self.rom.get(self.register[15] as usize / 2).cloned();
        self.register[15] = self.register[15].wrapping_add(2);
        inst
    }

    pub fn run_one_cycle(&mut self) {
        if self.is_halted() {
            return;
        }

        match self.in_flight_op {
            None => {
                let Some(inst) = self.read_rom_and_advance_pc() else {
                    self.set_halted();
                    return;
                };
                let ra = (inst >> 4 & 0x000F) as usize;
                let rb = (inst & 0x000F) as usize;
                let imm4 = inst & 0x000F;
                match inst {
                    // Noop
                    0x0000 => (),
                    // DBG
                    0x00BB => {
                        self.set_dbg_flag();
                    }
                    // HALT
                    0x0FFF => {
                        self.set_halted();
                    }
                    // LDW, LDB, LDS
                    0x1000..=0x1FFF => {
                        let signed = inst & 0x0800 != 0;
                        let byte_mode = inst & 0x0400 != 0;
                        let address_mode = inst >> 8 & 0x0003;
                        let mut addr = if rb == 0 { 0 } else { self.register[rb] };
                        if address_mode == 3 {
                            self.in_flight_op = Some(InFlightOp::LD_IMM {
                                signed,
                                byte_mode,
                                dst: ra,
                                base_addr: addr,
                            });
                        } else {
                            if address_mode == 2 {
                                addr = addr.wrapping_sub(if byte_mode { 1 } else { 2 });
                                self.register[rb] = addr;
                            }
                            let value = self.load_from_ram(addr, byte_mode, signed);
                            if address_mode == 1 {
                                addr = addr.wrapping_add(if byte_mode { 1 } else { 2 });
                                self.register[rb] = addr;
                            }
                            self.register[ra] = value;
                            self.set_flags_on_write(value);
                        }
                    }
                    // LRW, LRB, LRS
                    0x2000..=0x2FFF => {
                        let signed = inst & 0x0800 != 0;
                        let byte_mode = inst & 0x0400 != 0;
                        let address_mode = inst >> 8 & 0x0003;
                        let mut addr = if rb == 0 { 0 } else { self.register[rb] };
                        if address_mode == 3 {
                            self.in_flight_op = Some(InFlightOp::LR_IMM {
                                signed,
                                byte_mode,
                                dst: ra,
                                base_addr: addr,
                            })
                        } else {
                            if address_mode == 2 {
                                addr = addr.wrapping_sub(if byte_mode { 1 } else { 2 });
                                self.register[rb] = addr;
                            }
                            let raw = self.rom[addr as usize / 2];
                            let value = if byte_mode {
                                let value = if addr & 0x0001 != 0 {
                                    raw >> 8
                                } else {
                                    raw & 0x00FF
                                } as u8;
                                if signed {
                                    value as i8 as u16
                                } else {
                                    value as u16
                                }
                            } else {
                                raw
                            };
                            if address_mode == 1 {
                                addr = addr.wrapping_add(if byte_mode { 1 } else { 2 });
                                self.register[rb] = addr;
                            }
                            self.register[ra] = value;
                            self.set_flags_on_write(value);
                        }
                    }
                    // STW, STB
                    0x3000..=0x37FF => {
                        let byte_mode = inst & 0x0400 != 0;
                        let address_mode = inst >> 8 & 0x0003;
                        let mut addr = if ra == 0 { 0 } else { self.register[ra] };
                        let value = if rb == 0 { 0 } else { self.register[rb] };
                        if address_mode == 3 {
                            self.in_flight_op = Some(InFlightOp::ST_IMM {
                                byte_mode,
                                base_addr: addr,
                                value,
                            });
                        } else {
                            if address_mode == 2 {
                                addr = addr.wrapping_sub(if byte_mode { 1 } else { 2 });
                                self.register[ra] = addr;
                            }
                            self.write_ram(addr, value, byte_mode);
                            if address_mode == 1 {
                                addr = addr.wrapping_add(if byte_mode { 1 } else { 2 });
                                self.register[ra] = addr;
                            }
                        }
                    }
                    // LDI_IMM
                    0x4000..=0x40FF => {
                        self.in_flight_op = Some(InFlightOp::LDI_IMM { dst: ra });
                    }
                    // LDI_POS
                    0x4100..=0x41FF => {
                        self.register[ra] = imm4;
                        self.set_flags_on_write(imm4);
                    }
                    // LDI_NEG
                    0x4200..=0x42FF => {
                        let v = !imm4 + 1;
                        self.register[ra] = v;
                        self.set_flags_on_write(v);
                    }
                    // MOV
                    0x4300..=0x43FF => {
                        let v = if rb == 0 { 0 } else { self.register[rb] };
                        self.register[ra] = v;
                        self.set_flags_on_write(v);
                    }
                    // PUSH
                    0x4E00..=0x4EFF => {
                        self.register[14] = self.register[14].wrapping_sub(2);
                        let v = if ra == 0 { 0 } else { self.register[ra] };
                        let bytes = v.to_le_bytes();
                        self.ram[self.register[14] as usize] = bytes[0];
                        self.ram[self.register[14] as usize + 1] = bytes[1];
                    }
                    // POP
                    0x4F00..=0x4FFF => {
                        let sp = self.register[14] as usize;
                        let v = u16::from_le_bytes(self.ram[sp..sp + 2].try_into().unwrap());
                        self.register[ra] = v;
                        self.set_flags_on_write(v);
                        self.register[14] = self.register[14].wrapping_add(2);
                    }
                    //JMP_IMM
                    0x5000..=0x50FF => {
                        self.in_flight_op = Some(InFlightOp::JMP_IMM);
                    }
                    // JMP_REG
                    0x5100..=0x51FF => {
                        let a = if ra == 0 { 0 } else { self.register[ra] };
                        self.register[15] = a;
                    }
                    // JMP_OFF
                    0x5200..=0x52FF => {
                        let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
                        self.register[15] = self.register[15].wrapping_add(offset);
                    }
                    // JE
                    0x5300..=0x53FF => {
                        if self.is_zero_flag_set() {
                            let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
                            self.register[15] = self.register[15].wrapping_add(offset);
                        }
                    }
                    // JNE
                    0x5400..=0x54FF => {
                        if !self.is_zero_flag_set() {
                            let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
                            self.register[15] = self.register[15].wrapping_add(offset);
                        }
                    }
                    // JL
                    0x5500..=0x55FF => {
                        if self.is_negative_flag_set() != self.is_overflow_flag_set() {
                            let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
                            self.register[15] = self.register[15].wrapping_add(offset);
                        }
                    }
                    // JLE
                    0x5600..=0x56FF => {
                        if self.is_negative_flag_set() != self.is_overflow_flag_set()
                            || self.is_zero_flag_set()
                        {
                            let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
                            self.register[15] = self.register[15].wrapping_add(offset);
                        }
                    }
                    // JG
                    0x5700..=0x57FF => {
                        if self.is_negative_flag_set() == self.is_overflow_flag_set()
                            && !self.is_zero_flag_set()
                        {
                            let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
                            self.register[15] = self.register[15].wrapping_add(offset);
                        }
                    }
                    // JGE
                    0x5800..=0x58FF => {
                        if self.is_negative_flag_set() == self.is_overflow_flag_set() {
                            let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
                            self.register[15] = self.register[15].wrapping_add(offset);
                        }
                    }
                    // JB
                    0x5900..=0x59FF => {
                        if self.is_carry_flag_set() {
                            let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
                            self.register[15] = self.register[15].wrapping_add(offset);
                        }
                    }
                    // JBE
                    0x5A00..=0x5AFF => {
                        if self.is_carry_flag_set() || self.is_zero_flag_set() {
                            let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
                            self.register[15] = self.register[15].wrapping_add(offset);
                        }
                    }
                    // JA
                    0x5B00..=0x5BFF => {
                        if !self.is_carry_flag_set() && !self.is_zero_flag_set() {
                            let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
                            self.register[15] = self.register[15].wrapping_add(offset);
                        }
                    }
                    // JAE
                    0x5C00..=0x5CFF => {
                        if !self.is_carry_flag_set() {
                            let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
                            self.register[15] = self.register[15].wrapping_add(offset);
                        }
                    }
                    // ADD
                    0x6000..=0x60FF => {
                        let a = if ra == 0 { 0 } else { self.register[ra] };
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let (result, carry) = a.overflowing_add(b);
                        let overflow = ((a ^ result) & (b ^ result)) >> 15 != 0;
                        self.register[ra] = result;
                        self.set_flags(result == 0, result & 0x8000 != 0, carry, overflow);
                    }
                    // ADDC
                    0x6100..=0x61FF => {
                        let carry_value = if self.is_carry_flag_set() { 1 } else { 0 };
                        let a = if ra == 0 { 0 } else { self.register[ra] };
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let (sub_result, carry_1) = a.overflowing_add(carry_value);
                        let (result, carry_2) = sub_result.overflowing_add(b);
                        let overflow = ((a ^ result) & (b ^ result)) >> 15 != 0;
                        self.register[ra] = result;
                        self.set_flags(
                            result == 0,
                            result & 0x8000 != 0,
                            carry_2 || carry_1,
                            overflow,
                        );
                    }
                    // SUB
                    0x6200..=0x62FF => {
                        let a = if ra == 0 { 0 } else { self.register[ra] };
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let (result, carry) = a.overflowing_sub(b);
                        let overflow = (a ^ b) & (a ^ result) & 0x8000 != 0;
                        self.register[ra] = result;
                        self.set_flags(result == 0, result & 0x8000 != 0, carry, overflow);
                    }
                    // SUBC
                    0x6300..=0x63FF => {
                        let carry_value = if self.is_carry_flag_set() { 1 } else { 0 };
                        let a = if ra == 0 { 0 } else { self.register[ra] };
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let (sub_result, carry_1) = a.overflowing_sub(carry_value);
                        let (result, carry_2) = sub_result.overflowing_sub(b);
                        let overflow = (a ^ b) & (a ^ result) & 0x8000 != 0;
                        self.register[ra] = result;
                        self.set_flags(
                            result == 0,
                            result & 0x8000 != 0,
                            carry_2 || carry_1,
                            overflow,
                        );
                    }
                    // AND
                    0x6400..=0x64FF => {
                        let a = if ra == 0 { 0 } else { self.register[ra] };
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let result = a & b;
                        self.register[ra] = result;
                        self.set_flags(result == 0, result & 0x8000 != 0, false, false);
                    }
                    // Or
                    0x6500..=0x65FF => {
                        let a = if ra == 0 { 0 } else { self.register[ra] };
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let result = a | b;
                        self.register[ra] = result;
                        self.set_flags(result == 0, result & 0x8000 != 0, false, false);
                    }
                    // XOR
                    0x6600..=0x66FF => {
                        let a = if ra == 0 { 0 } else { self.register[ra] };
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let result = a ^ b;
                        self.register[ra] = result;
                        self.set_flags(result == 0, result & 0x8000 != 0, false, false);
                    }
                    // SHL
                    0x6700..=0x67FF => {
                        let a = if ra == 0 { 0 } else { self.register[ra] };
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let shift_amount = b & 0x000F;
                        let result = a << shift_amount;
                        self.register[ra] = result;
                        self.set_flags(
                            result == 0,
                            result & 0x8000 != 0,
                            a >> (16 - shift_amount) != 0,
                            (a & 0x8000) != (result & 0x8000),
                        );
                    }
                    // SHR
                    0x6800..=0x68FF => {
                        let a = if ra == 0 { 0 } else { self.register[ra] };
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let shift_amount = b & 0x000F;
                        let result = a >> shift_amount;
                        self.register[ra] = result;
                        self.set_flags(
                            result == 0,
                            false,
                            a << (16 - shift_amount) != 0,
                            (a & 0x8000) != (result & 0x8000),
                        );
                    }
                    // ASR
                    0x6900..=0x69FF => {
                        let a = if ra == 0 { 0 } else { self.register[ra] };
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let shift_amount = b & 0x000F;
                        let fill = 0xFFFF << (16 - shift_amount);
                        let result = (a >> shift_amount) | fill;
                        self.register[ra] = result;
                        self.set_flags(
                            result == 0,
                            false,
                            a << (16 - shift_amount) != 0,
                            (a & 0x8000) != (result & 0x8000),
                        );
                    }
                    // CMP
                    0x6A00..=0x6AFF => {
                        let a = if ra == 0 { 0 } else { self.register[ra] };
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let (result, carry) = a.overflowing_sub(b);
                        let overflow = (a ^ b) & (a ^ result) & 0x8000 != 0;
                        self.set_flags(result == 0, result & 0x8000 != 0, carry, overflow);
                    }
                    // NEG
                    0x6F00..=0x6F0F => {
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let result = (!b).wrapping_add(1);
                        self.register[rb] = result;
                        self.set_flags(result == 0, result & 0x8000 != 0, b & 0x8000 != 0, false);
                    }
                    // NOT
                    0x6F10..=0x6F1F => {
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let result = !b;
                        self.register[ra] = result;
                        self.set_flags(result == 0, result & 0x8000 != 0, false, false);
                    }
                    // INCB
                    0x6F20..=0x6F2F => {
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let (result, carry) = b.overflowing_add(1);
                        self.register[rb] = result;
                        self.set_flags(
                            result == 0,
                            result & 0x8000 != 0,
                            carry,
                            (result ^ b) & 0x8000 != 0,
                        );
                    }
                    // INCW
                    0x6F30..=0x6F3F => {
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let (result, carry) = b.overflowing_add(2);
                        self.register[rb] = result;
                        self.set_flags(
                            result == 0,
                            result & 0x8000 != 0,
                            carry,
                            (result ^ b) & 0x8000 != 0,
                        );
                    }
                    // DECB
                    0x6F40..=0x6F4F => {
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let (result, carry) = b.overflowing_sub(1);
                        self.register[rb] = result;
                        self.set_flags(
                            result == 0,
                            result & 0x8000 != 0,
                            carry,
                            (result ^ b) & 0x8000 != 0,
                        );
                    }
                    // DECW
                    0x6F50..=0x6F5F => {
                        let b = if rb == 0 { 0 } else { self.register[rb] };
                        let (result, carry) = b.overflowing_sub(2);
                        self.register[rb] = result;
                        self.set_flags(
                            result == 0,
                            result & 0x8000 != 0,
                            carry,
                            (result ^ b) & 0x8000 != 0,
                        );
                    }
                    _ => panic!("instruction {inst:#06x} not implemented"),
                }
                self.last_inst = Some(inst);
            }
            Some(InFlightOp::LDI_IMM { dst }) => {
                let Some(imm16) = self.read_rom_and_advance_pc() else {
                    self.set_halted();
                    return;
                };
                self.register[dst] = imm16;
                self.set_flags_on_write(imm16);
                self.in_flight_op = None;
            }
            Some(InFlightOp::JMP_IMM) => {
                let Some(addr) = self.read_rom_and_advance_pc() else {
                    self.set_halted();
                    return;
                };
                self.register[15] = addr;
                self.in_flight_op = None;
            }
            Some(InFlightOp::LD_IMM {
                signed,
                byte_mode,
                dst,
                base_addr,
            }) => {
                let Some(offset) = self.read_rom_and_advance_pc() else {
                    self.set_halted();
                    return;
                };
                let value = self.load_from_ram(base_addr.wrapping_add(offset), byte_mode, signed);
                self.register[dst] = value;
                self.set_flags_on_write(value);
                self.in_flight_op = None;
            }
            Some(InFlightOp::LR_IMM {
                signed,
                byte_mode,
                dst,
                base_addr,
            }) => {
                let Some(offset) = self.read_rom_and_advance_pc() else {
                    self.set_halted();
                    return;
                };
                let value = self.load_from_rom(base_addr.wrapping_add(offset), byte_mode, signed);
                self.register[dst] = value;
                self.set_flags_on_write(value);
            }
            Some(InFlightOp::ST_IMM {
                byte_mode,
                base_addr,
                value,
            }) => {
                let Some(offset) = self.read_rom_and_advance_pc() else {
                    self.set_halted();
                    return;
                };
                self.write_ram(base_addr.wrapping_add(offset), value, byte_mode);
                self.in_flight_op = None;
            }
        }
        self.cycle_counter = self.cycle_counter.wrapping_add(1);
    }

    fn load_from_ram(&self, addr: u16, byte_mode: bool, signed: bool) -> u16 {
        let raw = self.read_ram_byte(addr);
        if byte_mode {
            if signed { raw as i8 as u16 } else { raw as u16 }
        } else {
            raw as u16 | (self.read_ram_byte(addr.wrapping_add(1)) as u16) << 8
        }
    }

    fn read_ram_byte(&self, addr: u16) -> u8 {
        match addr {
            0xF000..=0xFFFF => 0,
            _ => self.ram[addr as usize],
        }
    }

    fn write_ram(&mut self, addr: u16, value: u16, byte_mode: bool) {
        let bytes = value.to_le_bytes();
        if byte_mode {
            self.write_ram_byte(addr, bytes[0]);
        } else {
            self.write_ram_byte(addr, bytes[0]);
            self.write_ram_byte(addr.wrapping_add(1), bytes[1]);
        }
    }

    fn write_ram_byte(&mut self, addr: u16, value: u8) {
        match addr {
            0xF000..=0xFFFF => (),
            _ => self.ram[addr as usize] = value,
        }
    }

    fn load_from_rom(&self, addr: u16, byte_mode: bool, signed: bool) -> u16 {
        let raw = self.rom[addr as usize / 2];
        if byte_mode {
            let value = if addr & 0x0001 != 0 {
                raw >> 8
            } else {
                raw & 0x00FF
            } as u8;
            if signed {
                value as i8 as u16
            } else {
                value as u16
            }
        } else {
            raw
        }
    }

    fn set_flags(&mut self, zero: bool, negative: bool, carry: bool, overflow: bool) {
        self.register[13] &= 0xFFF0;
        self.register[13] |= if zero { 0x0001 } else { 0 };
        self.register[13] |= if negative { 0x0002 } else { 0 };
        self.register[13] |= if carry { 0x0004 } else { 0 };
        self.register[13] |= if overflow { 0x0008 } else { 0 };
    }

    fn set_flags_on_write(&mut self, value: u16) {
        self.set_flags(value == 0, value & 0x8000 != 0, false, false);
    }

    pub fn is_halted(&self) -> bool {
        self.register[13] & 0x0040 != 0
    }

    pub fn set_halted(&mut self) {
        self.register[13] |= 0x0040;
    }

    pub fn unset_halted(&mut self) {
        self.register[13] &= !0x0040;
    }

    pub fn is_dbg_flag_set(&self) -> bool {
        self.register[13] & 0x0020 != 0
    }

    pub fn set_dbg_flag(&mut self) {
        self.register[13] |= 0x0020;
    }

    pub fn unset_dbg_flag(&mut self) {
        self.register[13] &= !0x0020;
    }

    pub fn is_zero_flag_set(&self) -> bool {
        self.register[13] & 0x0001 != 0
    }

    pub fn is_negative_flag_set(&self) -> bool {
        self.register[13] & 0x0002 != 0
    }

    pub fn is_carry_flag_set(&self) -> bool {
        self.register[13] & 0x0004 != 0
    }

    pub fn is_overflow_flag_set(&self) -> bool {
        self.register[13] & 0x0008 != 0
    }
    pub fn is_interrupt_request_enabled_flag_set(&self) -> bool {
        self.register[13] & 0x0010 != 0
    }

    pub fn print_status(&self, stdout: &mut Stdout) -> io::Result<()> {
        queue!(stdout, MoveTo(0, 0), Print("Registers:"))?;
        for i in 1..=12 {
            queue!(
                stdout,
                MoveTo(2, i),
                Print(format!(
                    "[R{:02}] = {:#06x} {:5} {:6}",
                    i,
                    self.register[i as usize],
                    self.register[i as usize],
                    self.register[i as usize] as i16,
                ))
            )?;
        }

        queue!(stdout, MoveTo(35, 0), Print("Flags:"))?;
        for (i, label) in vec!["ZF", "NF", "CF", "OF"].iter().enumerate() {
            let flag_state = (self.register[13] & (0x0001 << i)).count_ones();
            queue!(
                stdout,
                MoveTo(37, (1 + i) as u16),
                Print(format!("{}: {}", label, flag_state))
            )?;
        }
        for (i, label) in vec!["IF", "DF", "HF"].iter().enumerate() {
            let flag_state = (self.register[13] & (0x0010 << i)).count_ones();
            queue!(
                stdout,
                MoveTo(45, (1 + i) as u16),
                Print(format!("{}: {}", label, flag_state))
            )?;
        }

        queue!(
            stdout,
            MoveTo(35, 6),
            Print("Address Registers:"),
            MoveTo(37, 7),
            Print(format!("[SP] = {:#06x}", self.register[14])),
            MoveTo(37, 8),
            Print(format!("[PC] = {:#06x}", self.register[15])),
            MoveTo(35, 10),
            Print(format!("Cycles: {}", self.cycle_counter)),
            MoveTo(35, 11),
            Print(match self.last_inst {
                Some(inst) => format!("Last instruction: {:#06x}", inst),
                None => format!("Last instruction: ------"),
            }),
            MoveTo(35, 12),
            Print(match self.rom.get(self.register[15] as usize / 2) {
                Some(inst) => format!("Next instruction: {:#06x}", inst),
                None => format!("Next instruction: ------"),
            }),
        )?;

        queue!(stdout, MoveTo(0, 14), Print("RAM:"))?;
        let mut addr = 0;
        for line in 0..10 {
            queue!(
                stdout,
                MoveTo(0, 15 + line),
                Print(format!("{:4} | ", addr))
            )?;
            for i in 0..8 {
                queue!(stdout, Print(format!("{:02x} ", self.ram[addr + i])))?;
                if (i + 1) % 4 == 0 {
                    queue!(stdout, Print(" "))?;
                }
            }
            for i in 0..8 {
                let c = self.ram[addr + i] as char;

                queue!(
                    stdout,
                    Print(format!("{}", if !c.is_control() { c } else { '.' }))
                )?;
                if (i + 1) % 4 == 0 {
                    queue!(stdout, Print(" "))?;
                }
            }
            addr += 8;
        }

        queue!(stdout, MoveTo(45, 14), Print("RAM:"))?;

        Ok(())
    }
}
