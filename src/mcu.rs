use std::{
    io::{self, Stdout},
    sync::Arc,
    thread::current,
};

use crossterm::{cursor::MoveTo, queue, style::Print};
use tracing::warn;

use crate::isa::{
    self, AluOpcode, IfOpcode, InstructionFormat, MemoryOpcode, Misc, MiscOpcode, OffsetOpcode,
};

// #[allow(non_camel_case_types)]
// enum InFlightOp {
//     LDI_IMM {
//         dst: usize,
//     },
//     CALL_IMM,
//     JMP_IMM,
//     LD_IMM {
//         signed: bool,
//         byte_mode: bool,
//         dst: usize,
//         base_addr: u16,
//     },
//     LR_IMM {
//         signed: bool,
//         byte_mode: bool,
//         dst: usize,
//         base_addr: u16,
//     },
//     ST_IMM {
//         byte_mode: bool,
//         base_addr: u16,
//         value: u16,
//     },
// }
// Format misc:
pub const OPCODE_TRAP: u16 = 0x0000;
pub const OPCODE_CLI: u16 = 0x0100;
pub const OPCODE_STI: u16 = 0x0200;
pub const OPCODE_IRET: u16 = 0x0300;
pub const OPCODE_RET: u16 = 0x0400;
pub const OPCODE_CALL_IMM: u16 = 0x0500;
pub const OPCODE_DBG: u16 = 0x0600;
pub const OPCODE_HALT: u16 = 0x0700;
pub const OPCODE_INT: u16 = 0x0800;
pub const OPCODE_CALL_REG: u16 = 0x0900;
pub const OPCODE_NEG: u16 = 0x0A00;
// Format memory
pub const OPCODE_LDX: u16 = 0x1000;
pub const OPCODE_LRX: u16 = 0x2000;
pub const OPCODE_STX: u16 = 0x3000;
// Format alu
pub const OPCODE_ADD: u16 = 0x4000;
pub const OPCODE_ADDC: u16 = 0x4400;
pub const OPCODE_SUB: u16 = 0x4800;
pub const OPCODE_SUBC: u16 = 0x4C00;
pub const OPCODE_MUL: u16 = 0x5000;
pub const OPCODE_MULS: u16 = 0x5400;
pub const OPCODE_DIV: u16 = 0x5800;
pub const OPCODE_DIVS: u16 = 0x5C00;
pub const OPCODE_SHL: u16 = 0x6000;
pub const OPCODE_SHLC: u16 = 0x6400;
pub const OPCODE_SHR: u16 = 0x6800;
pub const OPCODE_SHRC: u16 = 0x6C00;
pub const OPCODE_SHA: u16 = 0x7000;
pub const OPCODE_MOV: u16 = 0x7400;
pub const OPCODE_AND: u16 = 0x7800;
pub const OPCODE_OR: u16 = 0x7C00;
pub const OPCODE_XOR: u16 = 0x8000;
pub const OPCODE_BCL: u16 = 0x8400;
// Format if
pub const OPCODE_IFE: u16 = 0x8800;
pub const OPCODE_IFNE: u16 = 0x8C00;
pub const OPCODE_IFL: u16 = 0x9000;
pub const OPCODE_IFLE: u16 = 0x9400;
pub const OPCODE_IFG: u16 = 0x9800;
pub const OPCODE_IFGE: u16 = 0x9C00;
pub const OPCODE_IFB: u16 = 0xA000;
pub const OPCODE_IFBE: u16 = 0xA400;
pub const OPCODE_IFA: u16 = 0xA800;
pub const OPCODE_IFAE: u16 = 0xAC00;
// Format offset
pub const OPCODE_JMP_OFF: u16 = 0xB000;
pub const OPCODE_CALL_OFF: u16 = 0xB400;

enum InstructionPhase {
    Fetch,
    FetchImmediateValue,
    Execute(u8),
}

pub struct MCU {
    pub register: [u16; 16],
    pub ram: [u8; u16::MAX as usize + 1],
    pub rom: Arc<Vec<u8>>,
    pub interrupts_disabled: bool,
    pub step_mode: bool,
    pub halted: bool,
    pub skip_flag: bool,
    pub carry: u16,
    pub current_inst: InstructionFormat,
    pub current_inst_cycles: i32,
    pub imm16: Option<u16>,
    pub last_inst: InstructionFormat,
    pub cycles: u32,
    pub serial_out: Vec<u8>,
}
impl MCU {
    pub fn new(program: Arc<Vec<u8>>) -> Self {
        Self {
            register: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xF000, 0],
            ram: [0; u16::MAX as usize + 1],
            rom: program,
            interrupts_disabled: false,
            step_mode: false,
            halted: false,
            skip_flag: false,
            carry: 0,
            cycles: 0,
            current_inst: InstructionFormat::trap(),
            current_inst_cycles: 0,
            imm16: None,
            last_inst: InstructionFormat::trap(),
            serial_out: Vec::new(),
        }
    }

    fn pop(&mut self) -> u16 {
        let sp = self.register[14] as usize;
        let v = u16::from_le_bytes(self.ram[sp..sp + 2].try_into().unwrap());
        self.register[14] = self.register[14].wrapping_add(2);
        v
    }

    fn push(&mut self, v: u16) {
        self.register[14] = self.register[14].wrapping_sub(2);
        let bytes = v.to_le_bytes();
        self.ram[self.register[14] as usize] = bytes[0];
        self.ram[self.register[14] as usize + 1] = bytes[1];
    }

    fn sp(&self) -> u16 {
        self.register[14]
    }

    fn postincrement_pc(&mut self) -> u16 {
        let pc = self.register[15];
        self.register[15] = pc.wrapping_add(2);
        pc
    }

    fn set_pc(&mut self, addr: u16) {
        self.register[15] = addr;
    }

    fn pc(&self) -> u16 {
        self.register[15]
    }

    fn fetch_next_rom_word(&mut self) -> u16 {
        let addr = self.postincrement_pc();
        self.read_rom(addr)
    }

    fn read_rom(&self, mut addr: u16) -> u16 {
        if addr % 2 != 0 {
            warn!("fetch from unaligned PC at addr: {addr}. Address is truncated into alignment");
            addr &= 0xFFFE;
        }
        u16::from_le_bytes([
            **&self.rom.get(addr as usize).unwrap_or(&0),
            **&self.rom.get(addr as usize + 1).unwrap_or(&0),
        ])
    }

    pub fn run_one_cycle(&mut self) {
        if self.halted {
            return;
        }

        if self.current_inst_cycles == 0 {
            self.current_inst = isa::decode_instruction(self.fetch_next_rom_word());
            if self.skip_flag {
                if self.current_inst.has_immediate_value() {
                    self.fetch_next_rom_word();
                }
                self.current_inst_cycles = 0;
                if self.current_inst.resets_skip_flag() {
                    self.skip_flag = false;
                }
                return;
            }
            self.current_inst_cycles = self.current_inst.cycles();
            self.imm16 = None;
            if self.current_inst.has_immediate_value() {
                return;
            } else {
                self.execute_current_instruction();
            }
        } else {
            if self.current_inst.has_immediate_value() && self.imm16.is_none() {
                self.imm16 = Some(self.fetch_next_rom_word());
            }
            self.execute_current_instruction();
        }

        match current_inst {
            0x0000..=0x0FFF => 'misc_block: {
                if self.skip_flag {
                    self.skip_flag = false;
                    break 'misc_block;
                }

                let op = current_inst & 0xFF00;
                match op {
                    OPCODE_TRAP => todo!(),
                    OPCODE_CLI => self.interrupts_disabled = false,
                    OPCODE_STI => self.interrupts_disabled = true,
                    OPCODE_IRET => todo!(),
                    OPCODE_RET => todo!(),
                    OPCODE_CALL_IMM => todo!(),
                    OPCODE_DBG => self.step_mode = true,
                    OPCODE_HALT => self.halted = true,
                    OPCODE_INT => todo!(),
                    OPCODE_CALL_REG => todo!(),
                    OPCODE_NEG => todo!(),
                    _ => todo!(),
                }
            }
            0x1000..=0x3FFF => 'memory_block: {
                let address_mode = decode_address_mode(current_inst);

                if self.skip_flag {
                    self.skip_flag = false;
                    if address_mode.has_immediate() {
                        self.set_pc(self.pc().wrapping_add(2));
                    }
                    break 'memory_block;
                }

                let addr = match address_mode {
                    AddressMode::Register(ri) => self.register[ri],
                    AddressMode::PostIncrement(ri, stride) => {
                        let addr = self.register[ri];
                        self.register[ri] = self.register[ri].wrapping_add(stride);
                        addr
                    }
                    AddressMode::PreDecrement(ri, stride) => {
                        self.register[ri] = self.register[ri].wrapping_sub(stride);
                        self.register[ri]
                    }
                    AddressMode::Offset(ri) => {
                        if self.current_inst_cycles == 0 {
                            self.current_inst_cycles += 1;
                            break 'memory_block;
                        }
                        self.register[ri].wrapping_add(self.fetch_next_rom_word())
                    }
                    AddressMode::Absolute => {
                        if self.current_inst_cycles == 0 {
                            self.current_inst_cycles += 1;
                            break 'memory_block;
                        }
                        self.fetch_next_rom_word()
                    }
                    AddressMode::Reserved => todo!(),
                };
                self.current_inst_cycles = 0;

                if self.skip_flag {
                    self.skip_flag = false;
                    break 'memory_block;
                }
                todo!()
            }
            0x4000..=0x7FFF => 'alu_block: {
                let p = match decode_parameter(current_inst) {
                    Parameter::Register(ri) => self.register[ri],
                    Parameter::Immediate => {
                        if self.current_inst_cycles == 0 {
                            self.current_inst_cycles += 1;
                            break 'alu_block;
                        }
                        self.fetch_next_rom_word()
                    }
                    Parameter::Value(p) => p,
                } as u32;
                self.current_inst_cycles = 0;

                if self.skip_flag {
                    self.skip_flag = false;
                    break 'alu_block;
                }

                let op = current_inst & 0xFC00;
                let ri = current_inst & 0xF;
                let r = self.register[ri as usize] as u32;
                let result = match op {
                    OPCODE_ADD => r + p,
                    OPCODE_ADDC => todo!(),
                    OPCODE_SUB => r - p,
                    OPCODE_SUBC => todo!(),
                    OPCODE_MUL => todo!(),
                    OPCODE_MULS => todo!(),
                    OPCODE_DIV => todo!(),
                    OPCODE_DIVS => todo!(),
                    OPCODE_SHL => {
                        let shift_amount = p & 0x7;
                        r << shift_amount
                    }
                    OPCODE_SHLC => todo!(),
                    OPCODE_SHR => {
                        let shift_amount = p & 0x7;
                        let result = r << (16 - shift_amount);
                        let carry = result & 0xFFF << 16;
                        result >> 16 | carry
                    }
                    OPCODE_SHRC => todo!(),
                    OPCODE_SHA => todo!(),
                    OPCODE_MOV => p,
                    OPCODE_AND => r & p,
                    OPCODE_OR => r | p,
                    OPCODE_XOR => r ^ p,
                    OPCODE_BCL => r & !p,
                    _ => todo!(),
                };
                self.register[ri as usize] = result as u16;
                self.carry = (result >> 16) as u16;
            }
            0x8000..=0xA7FF => 'if_block: {
                let p = match decode_parameter(current_inst) {
                    Parameter::Register(ri) => self.register[ri],
                    Parameter::Immediate => {
                        if self.current_inst_cycles == 0 {
                            self.current_inst_cycles += 1;
                            break 'if_block;
                        }
                        self.fetch_next_rom_word()
                    }
                    Parameter::Value(p) => p,
                };
                self.current_inst_cycles = 0;

                if self.skip_flag {
                    // if instructions dont reset the skip flag to allow chaining
                    break 'if_block;
                }
                let x = 0..1;

                let op = current_inst & 0xFC00;
                let r = self.register[(current_inst & 0xF) as usize];
                self.skip_flag = match op {
                    OPCODE_IFE => r == p,
                    OPCODE_IFNE => r != p,
                    OPCODE_IFL => (r as i16) < (p as i16),
                    OPCODE_IFLE => (r as i16) <= (p as i16),
                    OPCODE_IFG => (r as i16) > (p as i16),
                    OPCODE_IFGE => (r as i16) >= (p as i16),
                    OPCODE_IFB => r < p,
                    OPCODE_IFBE => r <= p,
                    OPCODE_IFA => r > p,
                    OPCODE_IFAE => r >= p,
                    _ => todo!(),
                };
            }
            0xA800..=0xACFF => 'offset_block: {
                if self.skip_flag {
                    self.skip_flag = false;
                    break 'offset_block;
                }

                match current_inst & 0xFC00 {
                    OPCODE_JMP_OFF => todo!(),
                    OPCODE_CALL_OFF => todo!(),
                    _ => todo!(),
                }
            }
            _ => {
                todo!("trap for invalid instruction")
            }
        };
    }

    pub fn execute_current_instruction(&mut self) {
        self.current_inst_cycles -= 1;
        if self.current_inst_cycles > 0 {
            return;
        }

        match &self.current_inst {
            InstructionFormat::Misc(misc) => match misc.opcode {
                MiscOpcode::TRAP => todo!(),
                MiscOpcode::CLI => todo!(),
                MiscOpcode::STI => todo!(),
                MiscOpcode::IRET => todo!(),
                MiscOpcode::RET => todo!(),
                MiscOpcode::CALLI => todo!(),
                MiscOpcode::DBG => todo!(),
                MiscOpcode::HALT => todo!(),
                MiscOpcode::INT => todo!(),
                MiscOpcode::CALLR => todo!(),
                MiscOpcode::NEG => todo!(),
            },
            InstructionFormat::Memory(memory) => match memory.opcode {
                MemoryOpcode::LDX => todo!(),
                MemoryOpcode::LRX => todo!(),
                MemoryOpcode::STX => todo!(),
            },
            InstructionFormat::Alu(alu) => {
                let p = match alu.parameter {
                    isa::Parameter::Register(ri) => self.register[ri],
                    isa::Parameter::Immediate => {
                        self.imm16.expect("Should have an immediate value")
                    }
                    isa::Parameter::Value(v) => v,
                };
                let r = self.register[alu.register];
                self.register[alu.register] = match alu.opcode {
                    AluOpcode::ADD => {
                        let (result, carry) = r.overflowing_add(p);
                        // TODO: handle the carry correctly
                        result
                    },
                    AluOpcode::ADDC => {
                        let (result_a, carry_a) = r.overflowing_add(p);
                        let (result_b, carry_b) = result_a.overflowing_add(self.carry);
                        // TODO: handle the carry correctly
                        // If either carry_a or carry_b is true the the carry is just 0b01
                        // If both are true then the carry is 0b10
                        result_b
                    },
                    AluOpcode::SUB => {
                        let (result, carry) = r.overflowing_sub(p);
                        // TODO: handle the carry correctly
                        result
                    },
                    AluOpcode::SUBC => todo!(),
                    AluOpcode::MUL => todo!(),
                    AluOpcode::MULS => todo!(),
                    AluOpcode::DIV => todo!(),
                    AluOpcode::DIVS => todo!(),
                    AluOpcode::SHL => todo!(),
                    AluOpcode::SHLC => todo!(),
                    AluOpcode::SHR => todo!(),
                    AluOpcode::SHRC => todo!(),
                    AluOpcode::SHA => todo!(),
                    AluOpcode::MOV => p,
                    AluOpcode::AND => r & p,
                    AluOpcode::OR => r | p,
                    AluOpcode::XOR => r ^ p,
                    AluOpcode::BCL => r & !p,
                }
            }
            InstructionFormat::If(iff) => {
                let p = match iff.parameter {
                    isa::Parameter::Register(ri) => self.register[ri],
                    isa::Parameter::Immediate => {
                        self.imm16.expect("Should have an immediate value")
                    }
                    isa::Parameter::Value(v) => v,
                };
                let r = self.register[iff.register];
                self.skip_flag = match iff.opcode {
                    IfOpcode::IFE => r == p,
                    IfOpcode::IFNE => r != p,
                    IfOpcode::IFL => (r as i16) < (p as i16),
                    IfOpcode::IFLE => (r as i16) <= (p as i16),
                    IfOpcode::IFG => (r as i16) > (p as i16),
                    IfOpcode::IFGE => (r as i16) >= (p as i16),
                    IfOpcode::IFB => r < p,
                    IfOpcode::IFBE => r <= p,
                    IfOpcode::IFA => r > p,
                    IfOpcode::IFAE => r >= p,
                };
            }
            InstructionFormat::Offset(offset) => match offset.opcode {
                OffsetOpcode::JMP => todo!(),
                OffsetOpcode::CALL => todo!(),
            },
            InstructionFormat::Error(_) => todo!(),
        }

        self.last_inst = self.current_inst;
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
            0xFFFF => self.serial_out.push(value),
            0xF000..=0xFFFF => (),
            _ => self.ram[addr as usize] = value,
        }
    }

    fn load_from_rom(&self, addr: u16, byte_mode: bool, signed: bool) -> u16 {
        let raw = self.read_rom(addr);
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
            Print(format!("Cycles: {}", self.cycles)),
            MoveTo(35, 11),
            Print(format!("Last instruction: {:#06x}", self.last_inst)),
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

enum Parameter {
    Register(usize),
    Immediate,
    Value(u16),
}

fn decode_parameter(instruction_word: u16) -> Parameter {
    let pm = instruction_word >> 8 & 0x3;
    let pv = instruction_word >> 4 & 0xF;
    match pm {
        0b00 => Parameter::Register(pv as usize),
        0b01 => match pv {
            0 => Parameter::Immediate,
            1 => Parameter::Value(-1 as i16 as u16),
            2 => Parameter::Value(0), // reserved
            3 => Parameter::Value(0x00FF),
            4 => Parameter::Value(0xFF00),
            5 => Parameter::Value(0x0020),
            6 => Parameter::Value(0x0040),
            7 => Parameter::Value(0x0080),
            8 => Parameter::Value(0x0100),
            9 => Parameter::Value(0x0200),
            10 => Parameter::Value(0x0400),
            11 => Parameter::Value(0x0800),
            12 => Parameter::Value(0x1000),
            13 => Parameter::Value(0x2000),
            14 => Parameter::Value(0x4000),
            15 => Parameter::Value(0x8000),
            _ => unreachable!(),
        },
        0b10 => Parameter::Value(pv),
        0b11 => Parameter::Value(16 + pv),
        _ => unreachable!(),
    }
}

enum AddressMode {
    Register(usize),
    PostIncrement(usize, u16),
    PreDecrement(usize, u16),
    Offset(usize),
    Absolute,
    Reserved,
}
impl AddressMode {
    fn has_immediate(&self) -> bool {
        match self {
            AddressMode::Register(_) => false,
            AddressMode::PostIncrement(_, _) => false,
            AddressMode::PreDecrement(_, _) => false,
            AddressMode::Offset(_) => true,
            AddressMode::Absolute => true,
            AddressMode::Reserved => false,
        }
    }
}
fn decode_address_mode(instruction_word: u16) -> AddressMode {
    let mode = instruction_word >> 8 & 0xF;
    let reg_a = (instruction_word >> 4 & 0xF) as usize;
    match mode {
        0 => AddressMode::Register(reg_a),
        1 => AddressMode::PostIncrement(reg_a, 2),
        2 => AddressMode::PreDecrement(reg_a, 2),
        3 => AddressMode::Offset(reg_a),
        4 => AddressMode::Absolute,
        5 => AddressMode::Register(reg_a),
        6 => AddressMode::PostIncrement(reg_a, 1),
        7 => AddressMode::PreDecrement(reg_a, 1),
        8 => AddressMode::Offset(reg_a),
        9 => AddressMode::Absolute,
        10 => AddressMode::Register(reg_a),
        11 => AddressMode::PostIncrement(reg_a, 1),
        12 => AddressMode::PreDecrement(reg_a, 1),
        13 => AddressMode::Offset(reg_a),
        14 => AddressMode::Absolute,
        15 => AddressMode::Reserved,
        _ => unreachable!(),
    }
}
