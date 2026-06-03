use std::{
    io::{self, Stdout},
    sync::Arc,
};

use crossterm::{cursor::MoveTo, queue, style::Print};
use tracing::warn;

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

pub struct MCU {
    pub register: [u16; 16],
    pub ram: [u8; u16::MAX as usize + 1],
    pub rom: Arc<Vec<u8>>,
    pub interrupts_disabled: bool,
    pub step_mode: bool,
    pub halted: bool,
    pub skip_flag: bool,
    pub carry: u16,
    pub last_inst: u16,
    pub instruction_phase: u8,
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
            last_inst: 0,
            instruction_phase: 0,
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

        let current_inst = if self.instruction_phase == 0 {
            self.fetch_next_rom_word()
        } else {
            self.last_inst
        };

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
                    if address_mode.has_immediate(){
                        self.set_pc(self.pc().wrapping_add(2));
                    }
                    break 'memory_block;
                }

                let addr = match address_mode{
                    AddressMode::Register(ri) => self.register[ri],
                    AddressMode::PostIncrement(ri, stride) => {
                        let addr = self.register[ri];
                        self.register[ri] = self.register[ri].wrapping_add(stride);
                        addr
                    },
                    AddressMode::PreDecrement(ri, stride) => {
                        self.register[ri] = self.register[ri].wrapping_sub(stride);
                        self.register[ri]
                    },
                    AddressMode::Offset(ri) => {
                        if self.instruction_phase == 0 {
                            self.instruction_phase += 1;
                            break 'memory_block;
                        }
                        self.register[ri].wrapping_add(self.fetch_next_rom_word())
                    },
                    AddressMode::Absolute => {
                        if self.instruction_phase == 0 {
                            self.instruction_phase += 1;
                            break 'memory_block;
                        }
                        self.fetch_next_rom_word()
                    },
                    AddressMode::Reserved => todo!(),
                };
                self.instruction_phase = 0;

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
                        if self.instruction_phase == 0 {
                            self.instruction_phase += 1;
                            break 'alu_block;
                        }
                        self.fetch_next_rom_word()
                    }
                    Parameter::Value(p) => p,
                } as u32;
                self.instruction_phase = 0;

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
                        if self.instruction_phase == 0 {
                            self.instruction_phase += 1;
                            break 'if_block;
                        }
                        self.fetch_next_rom_word()
                    }
                    Parameter::Value(p) => p,
                };
                self.instruction_phase = 0;

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

        self.last_inst = current_inst;
    }

    // pub fn run_one_cycle_old(&mut self) {
    //     if self.is_halted() {
    //         return;
    //     }
    //
    //     match self.in_flight_op {
    //         None => {
    //             let inst = self.fetch_next_rom_word();
    //             let ra = (inst >> 4 & 0x000F) as usize;
    //             let rb = (inst & 0x000F) as usize;
    //             let imm4 = inst & 0x000F;
    //             match inst {
    //                 // Noop
    //                 0x0000 => (),
    //                 // CALL imm
    //                 0x0010..=0x001F => self.in_flight_op = Some(InFlightOp::CALL_IMM),
    //                 // CALL reg
    //                 0x0020..=0x002F => {
    //                     let a = if ra == 0 { 0 } else { self.register[ra] };
    //                     self.register[14] = self.register[14].wrapping_sub(2);
    //                     let bytes = self.register[15].to_le_bytes();
    //                     self.ram[self.register[14] as usize] = bytes[0];
    //                     self.ram[self.register[14] as usize + 1] = bytes[1];
    //                     self.register[15] = a;
    //                 }
    //                 // RET
    //                 0x0030..=0x003F => {
    //                     let sp = self.register[14] as usize;
    //                     let v = u16::from_le_bytes(self.ram[sp..sp + 2].try_into().unwrap());
    //                     self.register[15] = v;
    //                     self.set_flags_on_write(v);
    //                     self.register[14] = self.register[14].wrapping_add(2);
    //                 }
    //                 // DBG
    //                 0x00BB => {
    //                     self.set_dbg_flag();
    //                 }
    //                 // HALT
    //                 0x0FFF => {
    //                     self.set_halted();
    //                 }
    //                 // LDW, LDB, LDS
    //                 0x1000..=0x1FFF => {
    //                     let signed = inst & 0x0800 != 0;
    //                     let byte_mode = inst & 0x0400 != 0;
    //                     let address_mode = inst >> 8 & 0x0003;
    //                     let mut addr = if rb == 0 { 0 } else { self.register[rb] };
    //                     if address_mode == 3 {
    //                         self.in_flight_op = Some(InFlightOp::LD_IMM {
    //                             signed,
    //                             byte_mode,
    //                             dst: ra,
    //                             base_addr: addr,
    //                         });
    //                     } else {
    //                         if address_mode == 2 {
    //                             addr = addr.wrapping_sub(if byte_mode { 1 } else { 2 });
    //                             self.register[rb] = addr;
    //                         }
    //                         let value = self.load_from_ram(addr, byte_mode, signed);
    //                         if address_mode == 1 {
    //                             addr = addr.wrapping_add(if byte_mode { 1 } else { 2 });
    //                             self.register[rb] = addr;
    //                         }
    //                         self.register[ra] = value;
    //                         self.set_flags_on_write(value);
    //                     }
    //                 }
    //                 // LRW, LRB, LRS
    //                 0x2000..=0x2FFF => {
    //                     let signed = inst & 0x0800 != 0;
    //                     let byte_mode = inst & 0x0400 != 0;
    //                     let address_mode = inst >> 8 & 0x0003;
    //                     let mut addr = if rb == 0 { 0 } else { self.register[rb] };
    //                     if address_mode == 3 {
    //                         self.in_flight_op = Some(InFlightOp::LR_IMM {
    //                             signed,
    //                             byte_mode,
    //                             dst: ra,
    //                             base_addr: addr,
    //                         })
    //                     } else {
    //                         if address_mode == 2 {
    //                             addr = addr.wrapping_sub(if byte_mode { 1 } else { 2 });
    //                             self.register[rb] = addr;
    //                         }
    //                         let raw = self.read_rom(addr);
    //                         let value = if byte_mode {
    //                             let value = if addr & 0x0001 != 0 {
    //                                 raw >> 8
    //                             } else {
    //                                 raw & 0x00FF
    //                             } as u8;
    //                             if signed {
    //                                 value as i8 as u16
    //                             } else {
    //                                 value as u16
    //                             }
    //                         } else {
    //                             raw
    //                         };
    //                         if address_mode == 1 {
    //                             addr = addr.wrapping_add(if byte_mode { 1 } else { 2 });
    //                             self.register[rb] = addr;
    //                         }
    //                         self.register[ra] = value;
    //                         self.set_flags_on_write(value);
    //                     }
    //                 }
    //                 // STW, STB
    //                 0x3000..=0x37FF => {
    //                     let byte_mode = inst & 0x0400 != 0;
    //                     let address_mode = inst >> 8 & 0x0003;
    //                     let mut addr = if ra == 0 { 0 } else { self.register[ra] };
    //                     let value = if rb == 0 { 0 } else { self.register[rb] };
    //                     if address_mode == 3 {
    //                         self.in_flight_op = Some(InFlightOp::ST_IMM {
    //                             byte_mode,
    //                             base_addr: addr,
    //                             value,
    //                         });
    //                     } else {
    //                         if address_mode == 2 {
    //                             addr = addr.wrapping_sub(if byte_mode { 1 } else { 2 });
    //                             self.register[ra] = addr;
    //                         }
    //                         self.write_ram(addr, value, byte_mode);
    //                         if address_mode == 1 {
    //                             addr = addr.wrapping_add(if byte_mode { 1 } else { 2 });
    //                             self.register[ra] = addr;
    //                         }
    //                     }
    //                 }
    //                 // LDI_IMM
    //                 0x4000..=0x40FF => {
    //                     self.in_flight_op = Some(InFlightOp::LDI_IMM { dst: ra });
    //                 }
    //                 // LDI_POS
    //                 0x4100..=0x41FF => {
    //                     self.register[ra] = imm4;
    //                     self.set_flags_on_write(imm4);
    //                 }
    //                 // LDI_NEG
    //                 0x4200..=0x42FF => {
    //                     let v = !imm4 + 1;
    //                     self.register[ra] = v;
    //                     self.set_flags_on_write(v);
    //                 }
    //                 // MOV
    //                 0x4300..=0x43FF => {
    //                     let v = if rb == 0 { 0 } else { self.register[rb] };
    //                     self.register[ra] = v;
    //                     self.set_flags_on_write(v);
    //                 }
    //                 // PUSH
    //                 0x4E00..=0x4EFF => {
    //                     self.register[14] = self.register[14].wrapping_sub(2);
    //                     let v = if ra == 0 { 0 } else { self.register[ra] };
    //                     let bytes = v.to_le_bytes();
    //                     self.ram[self.register[14] as usize] = bytes[0];
    //                     self.ram[self.register[14] as usize + 1] = bytes[1];
    //                 }
    //                 // POP
    //                 0x4F00..=0x4FFF => {
    //                     let sp = self.register[14] as usize;
    //                     let v = u16::from_le_bytes(self.ram[sp..sp + 2].try_into().unwrap());
    //                     self.register[ra] = v;
    //                     self.set_flags_on_write(v);
    //                     self.register[14] = self.register[14].wrapping_add(2);
    //                 }
    //                 //JMP_IMM
    //                 0x5000..=0x50FF => {
    //                     self.in_flight_op = Some(InFlightOp::JMP_IMM);
    //                 }
    //                 // JMP_REG
    //                 0x5100..=0x51FF => {
    //                     let a = if ra == 0 { 0 } else { self.register[ra] };
    //                     self.register[15] = a;
    //                 }
    //                 // JMP_OFF
    //                 0x5200..=0x52FF => {
    //                     let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
    //                     self.register[15] = self.register[15].wrapping_add(offset);
    //                 }
    //                 // JE
    //                 0x5300..=0x53FF => {
    //                     if self.is_zero_flag_set() {
    //                         let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
    //                         self.register[15] = self.register[15].wrapping_add(offset);
    //                     }
    //                 }
    //                 // JNE
    //                 0x5400..=0x54FF => {
    //                     if !self.is_zero_flag_set() {
    //                         let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
    //                         self.register[15] = self.register[15].wrapping_add(offset);
    //                     }
    //                 }
    //                 // JL
    //                 0x5500..=0x55FF => {
    //                     if self.is_negative_flag_set() != self.is_overflow_flag_set() {
    //                         let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
    //                         self.register[15] = self.register[15].wrapping_add(offset);
    //                     }
    //                 }
    //                 // JLE
    //                 0x5600..=0x56FF => {
    //                     if self.is_negative_flag_set() != self.is_overflow_flag_set()
    //                         || self.is_zero_flag_set()
    //                     {
    //                         let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
    //                         self.register[15] = self.register[15].wrapping_add(offset);
    //                     }
    //                 }
    //                 // JG
    //                 0x5700..=0x57FF => {
    //                     if self.is_negative_flag_set() == self.is_overflow_flag_set()
    //                         && !self.is_zero_flag_set()
    //                     {
    //                         let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
    //                         self.register[15] = self.register[15].wrapping_add(offset);
    //                     }
    //                 }
    //                 // JGE
    //                 0x5800..=0x58FF => {
    //                     if self.is_negative_flag_set() == self.is_overflow_flag_set() {
    //                         let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
    //                         self.register[15] = self.register[15].wrapping_add(offset);
    //                     }
    //                 }
    //                 // JB
    //                 0x5900..=0x59FF => {
    //                     if self.is_carry_flag_set() {
    //                         let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
    //                         self.register[15] = self.register[15].wrapping_add(offset);
    //                     }
    //                 }
    //                 // JBE
    //                 0x5A00..=0x5AFF => {
    //                     if self.is_carry_flag_set() || self.is_zero_flag_set() {
    //                         let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
    //                         self.register[15] = self.register[15].wrapping_add(offset);
    //                     }
    //                 }
    //                 // JA
    //                 0x5B00..=0x5BFF => {
    //                     if !self.is_carry_flag_set() && !self.is_zero_flag_set() {
    //                         let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
    //                         self.register[15] = self.register[15].wrapping_add(offset);
    //                     }
    //                 }
    //                 // JAE
    //                 0x5C00..=0x5CFF => {
    //                     if !self.is_carry_flag_set() {
    //                         let offset = ((inst & 0x00FF) as i8 as i16 * 2) as u16;
    //                         self.register[15] = self.register[15].wrapping_add(offset);
    //                     }
    //                 }
    //                 // ADD
    //                 0x6000..=0x60FF => {
    //                     let a = if ra == 0 { 0 } else { self.register[ra] };
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let (result, carry) = a.overflowing_add(b);
    //                     let overflow = ((a ^ result) & (b ^ result)) >> 15 != 0;
    //                     self.register[ra] = result;
    //                     self.set_flags(result == 0, result & 0x8000 != 0, carry, overflow);
    //                 }
    //                 // ADDC
    //                 0x6100..=0x61FF => {
    //                     let carry_value = if self.is_carry_flag_set() { 1 } else { 0 };
    //                     let a = if ra == 0 { 0 } else { self.register[ra] };
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let (sub_result, carry_1) = a.overflowing_add(carry_value);
    //                     let (result, carry_2) = sub_result.overflowing_add(b);
    //                     let overflow = ((a ^ result) & (b ^ result)) >> 15 != 0;
    //                     self.register[ra] = result;
    //                     self.set_flags(
    //                         result == 0,
    //                         result & 0x8000 != 0,
    //                         carry_2 || carry_1,
    //                         overflow,
    //                     );
    //                 }
    //                 // SUB
    //                 0x6200..=0x62FF => {
    //                     let a = if ra == 0 { 0 } else { self.register[ra] };
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let (result, carry) = a.overflowing_sub(b);
    //                     let overflow = (a ^ b) & (a ^ result) & 0x8000 != 0;
    //                     self.register[ra] = result;
    //                     self.set_flags(result == 0, result & 0x8000 != 0, carry, overflow);
    //                 }
    //                 // SUBC
    //                 0x6300..=0x63FF => {
    //                     let carry_value = if self.is_carry_flag_set() { 1 } else { 0 };
    //                     let a = if ra == 0 { 0 } else { self.register[ra] };
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let (sub_result, carry_1) = a.overflowing_sub(carry_value);
    //                     let (result, carry_2) = sub_result.overflowing_sub(b);
    //                     let overflow = (a ^ b) & (a ^ result) & 0x8000 != 0;
    //                     self.register[ra] = result;
    //                     self.set_flags(
    //                         result == 0,
    //                         result & 0x8000 != 0,
    //                         carry_2 || carry_1,
    //                         overflow,
    //                     );
    //                 }
    //                 // AND
    //                 0x6400..=0x64FF => {
    //                     let a = if ra == 0 { 0 } else { self.register[ra] };
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let result = a & b;
    //                     self.register[ra] = result;
    //                     self.set_flags(result == 0, result & 0x8000 != 0, false, false);
    //                 }
    //                 // Or
    //                 0x6500..=0x65FF => {
    //                     let a = if ra == 0 { 0 } else { self.register[ra] };
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let result = a | b;
    //                     self.register[ra] = result;
    //                     self.set_flags(result == 0, result & 0x8000 != 0, false, false);
    //                 }
    //                 // XOR
    //                 0x6600..=0x66FF => {
    //                     let a = if ra == 0 { 0 } else { self.register[ra] };
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let result = a ^ b;
    //                     self.register[ra] = result;
    //                     self.set_flags(result == 0, result & 0x8000 != 0, false, false);
    //                 }
    //                 // SHL
    //                 0x6700..=0x67FF => {
    //                     let a = if ra == 0 { 0 } else { self.register[ra] };
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let shift_amount = b & 0x000F;
    //                     let result = a << shift_amount;
    //                     self.register[ra] = result;
    //                     self.set_flags(
    //                         result == 0,
    //                         result & 0x8000 != 0,
    //                         a >> (16 - shift_amount) != 0,
    //                         (a & 0x8000) != (result & 0x8000),
    //                     );
    //                 }
    //                 // SHR
    //                 0x6800..=0x68FF => {
    //                     let a = if ra == 0 { 0 } else { self.register[ra] };
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let shift_amount = b & 0x000F;
    //                     let result = a >> shift_amount;
    //                     self.register[ra] = result;
    //                     self.set_flags(
    //                         result == 0,
    //                         false,
    //                         a.unbounded_shl((16 - shift_amount) as u32) != 0,
    //                         (a & 0x8000) != (result & 0x8000),
    //                     );
    //                 }
    //                 // ASR
    //                 0x6900..=0x69FF => {
    //                     let a = if ra == 0 { 0 } else { self.register[ra] };
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let shift_amount = b & 0x000F;
    //                     let fill = 0xFFFF << (16 - shift_amount);
    //                     let result = (a >> shift_amount) | fill;
    //                     self.register[ra] = result;
    //                     self.set_flags(
    //                         result == 0,
    //                         false,
    //                         a << (16 - shift_amount) != 0,
    //                         (a & 0x8000) != (result & 0x8000),
    //                     );
    //                 }
    //                 // CMP
    //                 0x6A00..=0x6AFF => {
    //                     let a = if ra == 0 { 0 } else { self.register[ra] };
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let (result, carry) = a.overflowing_sub(b);
    //                     let overflow = (a ^ b) & (a ^ result) & 0x8000 != 0;
    //                     self.set_flags(result == 0, result & 0x8000 != 0, carry, overflow);
    //                 }
    //                 // NEG
    //                 0x6F00..=0x6F0F => {
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let result = (!b).wrapping_add(1);
    //                     self.register[rb] = result;
    //                     self.set_flags(result == 0, result & 0x8000 != 0, b & 0x8000 != 0, false);
    //                 }
    //                 // NOT
    //                 0x6F10..=0x6F1F => {
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let result = !b;
    //                     self.register[ra] = result;
    //                     self.set_flags(result == 0, result & 0x8000 != 0, false, false);
    //                 }
    //                 // INCB
    //                 0x6F20..=0x6F2F => {
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let (result, carry) = b.overflowing_add(1);
    //                     self.register[rb] = result;
    //                     self.set_flags(
    //                         result == 0,
    //                         result & 0x8000 != 0,
    //                         carry,
    //                         (result ^ b) & 0x8000 != 0,
    //                     );
    //                 }
    //                 // INCW
    //                 0x6F30..=0x6F3F => {
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let (result, carry) = b.overflowing_add(2);
    //                     self.register[rb] = result;
    //                     self.set_flags(
    //                         result == 0,
    //                         result & 0x8000 != 0,
    //                         carry,
    //                         (result ^ b) & 0x8000 != 0,
    //                     );
    //                 }
    //                 // DECB
    //                 0x6F40..=0x6F4F => {
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let (result, carry) = b.overflowing_sub(1);
    //                     self.register[rb] = result;
    //                     self.set_flags(
    //                         result == 0,
    //                         result & 0x8000 != 0,
    //                         carry,
    //                         (result ^ b) & 0x8000 != 0,
    //                     );
    //                 }
    //                 // DECW
    //                 0x6F50..=0x6F5F => {
    //                     let b = if rb == 0 { 0 } else { self.register[rb] };
    //                     let (result, carry) = b.overflowing_sub(2);
    //                     self.register[rb] = result;
    //                     self.set_flags(
    //                         result == 0,
    //                         result & 0x8000 != 0,
    //                         carry,
    //                         (result ^ b) & 0x8000 != 0,
    //                     );
    //                 }
    //                 _ => panic!("instruction {inst:#06x} not implemented"),
    //             }
    //             self.last_inst = inst;
    //         }
    //         Some(InFlightOp::LDI_IMM { dst }) => {
    //             let imm16 = self.fetch_next_rom_word();
    //             self.register[dst] = imm16;
    //             self.set_flags_on_write(imm16);
    //             self.in_flight_op = None;
    //         }
    //         Some(InFlightOp::CALL_IMM) => {
    //             let addr = self.fetch_next_rom_word();
    //             self.register[14] = self.register[14].wrapping_sub(2);
    //             let bytes = self.register[15].to_le_bytes();
    //             self.ram[self.register[14] as usize] = bytes[0];
    //             self.ram[self.register[14] as usize + 1] = bytes[1];
    //             self.register[15] = addr;
    //             self.in_flight_op = None;
    //         }
    //         Some(InFlightOp::JMP_IMM) => {
    //             let addr = self.fetch_next_rom_word();
    //             self.register[15] = addr;
    //             self.in_flight_op = None;
    //         }
    //         Some(InFlightOp::LD_IMM {
    //             signed,
    //             byte_mode,
    //             dst,
    //             base_addr,
    //         }) => {
    //             let offset = self.fetch_next_rom_word();
    //             let value = self.load_from_ram(base_addr.wrapping_add(offset), byte_mode, signed);
    //             self.register[dst] = value;
    //             self.set_flags_on_write(value);
    //             self.in_flight_op = None;
    //         }
    //         Some(InFlightOp::LR_IMM {
    //             signed,
    //             byte_mode,
    //             dst,
    //             base_addr,
    //         }) => {
    //             let offset = self.fetch_next_rom_word();
    //             let value = self.load_from_rom(base_addr.wrapping_add(offset), byte_mode, signed);
    //             self.register[dst] = value;
    //             self.set_flags_on_write(value);
    //         }
    //         Some(InFlightOp::ST_IMM {
    //             byte_mode,
    //             base_addr,
    //             value,
    //         }) => {
    //             let offset = self.fetch_next_rom_word();
    //             self.write_ram(base_addr.wrapping_add(offset), value, byte_mode);
    //             self.in_flight_op = None;
    //         }
    //     }
    //     self.cycles = self.cycles.wrapping_add(1);
    // }

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

enum AddressMode{
    Register(usize),
    PostIncrement(usize, u16),
    PreDecrement(usize, u16),
    Offset(usize),
    Absolute,
    Reserved
}
impl AddressMode{
    fn has_immediate(&self) -> bool {
        match self{
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