use ux::u4;

#[allow(non_camel_case_types)]
pub enum Instruction {
    NOP,
    CALL_IMM(u16),
    CALL_REG(Register),
    RET,
    INT(u8),
    IRET,
    CLI,
    STI,
    DBG,
    HALT,
    LDW(Register, Address),
    LDB(Register, Address),
    LDS(Register, Address),
    LRW(Register, Address),
    LRB(Register, Address),
    LRS(Register, Address),
    STW(Address, Register),
    STB(Address, Register),
    LDI_IMM(Register, u16),
    LDI_POS(Register, u4),
    LDI_NEG(Register, u4),
    MOV(Register, Register),
    PUSH(Register),
    POP(Register),
    JMP_IMM(u16),
    JMP_REG(Register),
    JMP_OFF(i8),
    JE(i8),
    JNE(i8),
    JL(i8),
    JLE(i8),
    JG(i8),
    JGE(i8),
    JB(i8),
    JBE(i8),
    JA(i8),
    JAE(i8),
    ADD(Register, Register),
    ADDC(Register, Register),
    SUB(Register, Register),
    SUBC(Register, Register),
    AND(Register, Register),
    OR(Register, Register),
    XOR(Register, Register),
    SHL(Register, Register),
    SHR(Register, Register),
    ASR(Register, Register),
    CMP(Register, Register),
    NOT(Register),
    NEG(Register),
    INCB(Register),
    INCW(Register),
    DECB(Register),
    DECW(Register),
    IMM(u16),
}

pub enum Register {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
    R9,
    R10,
    R11,
    R12,
    ST,
    SP,
    PC,
}
impl Register {
    pub fn id(&self) -> usize {
        match self {
            Register::R0 => 0,
            Register::R1 => 1,
            Register::R2 => 2,
            Register::R3 => 3,
            Register::R4 => 4,
            Register::R5 => 5,
            Register::R6 => 6,
            Register::R7 => 7,
            Register::R8 => 7,
            Register::R9 => 9,
            Register::R10 => 10,
            Register::R11 => 11,
            Register::R12 => 12,
            Register::ST => 13,
            Register::SP => 14,
            Register::PC => 15,
        }
    }
    pub fn as_first_param(&self) -> u16 {
        (self.id() as u16) << 4
    }
    pub fn as_second_param(&self) -> u16 {
        self.id() as u16
    }
}

pub enum Address {
    Indirect(Register),
    PostIncrement(Register),
    PreDecrement(Register),
    Offset(Register, u16),
}
impl Address {
    fn register(&self) -> &Register {
        match self {
            Address::Indirect(register) => register,
            Address::PostIncrement(register) => register,
            Address::PreDecrement(register) => register,
            Address::Offset(register, _) => register,
        }
    }
    fn mode(&self) -> u16 {
        match self {
            Address::Indirect(_) => 0x0000,
            Address::PostIncrement(_) => 0x0100,
            Address::PreDecrement(_) => 0x0200,
            Address::Offset(_, _) => 0x0300,
        }
    }
}

pub fn to_bytes(program: Vec<Instruction>) -> Vec<u16> {
    let mut output = Vec::new();

    for inst in program {
        match inst {
            Instruction::NOP => {
                output.push(0x0000);
            }
            Instruction::CALL_IMM(_) => todo!(),
            Instruction::CALL_REG(register) => todo!(),
            Instruction::RET => todo!(),
            Instruction::INT(_) => todo!(),
            Instruction::IRET => todo!(),
            Instruction::CLI => todo!(),
            Instruction::STI => todo!(),
            Instruction::DBG => {
                output.push(0x00BB);
            }
            Instruction::HALT => {
                output.push(0x0FFF);
            }
            Instruction::LDW(dst, address) => {
                output.push(
                    0x1000
                        + address.mode()
                        + dst.as_first_param()
                        + address.register().as_second_param(),
                );
                if let Address::Offset(_, offset) = address {
                    output.push(offset);
                }
            }
            Instruction::LDB(dst, address) => {
                output.push(
                    0x1400
                        + address.mode()
                        + dst.as_first_param()
                        + address.register().as_second_param(),
                );
                if let Address::Offset(_, offset) = address {
                    output.push(offset);
                }
            }
            Instruction::LDS(dst, address) => {
                output.push(
                    0x1C00
                        + address.mode()
                        + dst.as_first_param()
                        + address.register().as_second_param(),
                );
                if let Address::Offset(_, offset) = address {
                    output.push(offset);
                }
            }
            Instruction::LRW(dst, address) => {
                output.push(
                    0x2000
                        + address.mode()
                        + dst.as_first_param()
                        + address.register().as_second_param(),
                );
                if let Address::Offset(_, offset) = address {
                    output.push(offset);
                }
            }
            Instruction::LRB(dst, address) => {
                output.push(
                    0x2400
                        + address.mode()
                        + dst.as_first_param()
                        + address.register().as_second_param(),
                );
                if let Address::Offset(_, offset) = address {
                    output.push(offset);
                }
            }
            Instruction::LRS(dst, address) => {
                output.push(
                    0x2C00
                        + address.mode()
                        + dst.as_first_param()
                        + address.register().as_second_param(),
                );
                if let Address::Offset(_, offset) = address {
                    output.push(offset);
                }
            }
            Instruction::STW(address, src) => {
                output.push(
                    0x3000
                        + address.mode()
                        + address.register().as_first_param()
                        + src.as_second_param(),
                );
                if let Address::Offset(_, offset) = address {
                    output.push(offset);
                }
            }
            Instruction::STB(address, src) => {
                output.push(
                    0x3400
                        + address.mode()
                        + address.register().as_first_param()
                        + src.as_second_param(),
                );
                if let Address::Offset(_, offset) = address {
                    output.push(offset);
                }
            }
            Instruction::LDI_IMM(dst, v) => {
                output.push(0x4000 + dst.as_first_param());
                output.push(v);
            }
            Instruction::LDI_POS(dst, v) => {
                output.push(0x4100 + dst.as_first_param() + u16::from(v));
            }
            Instruction::LDI_NEG(dst, v) => {
                output.push(0x4200 + dst.as_first_param() + u16::from(v));
            }
            Instruction::MOV(dst, src) => {
                output.push(0x4300 + dst.as_first_param() + src.as_second_param());
            }
            Instruction::PUSH(src) => {
                output.push(0x4E00 + src.as_first_param());
            }
            Instruction::POP(dst) => {
                output.push(0x4F00 + dst.as_first_param());
            }
            Instruction::JMP_IMM(addr) => {
                output.push(0x5000);
                output.push(addr);
            }
            Instruction::JMP_REG(src) => {
                output.push(0x5100 + src.as_first_param());
            }
            Instruction::JMP_OFF(offset) => {
                output.push(0x5200 + (offset as u8) as u16);
            }
            Instruction::JE(offset) => {
                output.push(0x5300 + (offset as u8) as u16);
            }
            Instruction::JNE(offset) => {
                output.push(0x5400 + (offset as u8) as u16);
            }
            Instruction::JL(offset) => {
                output.push(0x5500 + (offset as u8) as u16);
            }
            Instruction::JLE(offset) => {
                output.push(0x5600 + (offset as u8) as u16);
            }
            Instruction::JG(offset) => {
                output.push(0x5700 + (offset as u8) as u16);
            }
            Instruction::JGE(offset) => {
                output.push(0x5800 + (offset as u8) as u16);
            }
            Instruction::JB(offset) => {
                output.push(0x5900 + (offset as u8) as u16);
            }
            Instruction::JBE(offset) => {
                output.push(0x5A00 + (offset as u8) as u16);
            }
            Instruction::JA(offset) => {
                output.push(0x5B00 + (offset as u8) as u16);
            }
            Instruction::JAE(offset) => {
                output.push(0x5C00 + (offset as u8) as u16);
            }
            Instruction::ADD(a, b) => {
                output.push(0x6000 + a.as_first_param() + b.as_second_param());
            }
            Instruction::ADDC(a, b) => {
                output.push(0x6100 + a.as_first_param() + b.as_second_param());
            }
            Instruction::SUB(a, b) => {
                output.push(0x6200 + a.as_first_param() + b.as_second_param());
            }
            Instruction::SUBC(a, b) => {
                output.push(0x6300 + a.as_first_param() + b.as_second_param());
            }
            Instruction::AND(a, b) => {
                output.push(0x6400 + a.as_first_param() + b.as_second_param());
            }
            Instruction::OR(a, b) => {
                output.push(0x6500 + a.as_first_param() + b.as_second_param());
            }
            Instruction::XOR(a, b) => {
                output.push(0x6600 + a.as_first_param() + b.as_second_param());
            }
            Instruction::SHL(a, b) => {
                output.push(0x6700 + a.as_first_param() + b.as_second_param());
            }
            Instruction::SHR(a, b) => {
                output.push(0x6800 + a.as_first_param() + b.as_second_param());
            }
            Instruction::ASR(a, b) => {
                output.push(0x6900 + a.as_first_param() + b.as_second_param());
            }
            Instruction::CMP(a, b) => {
                output.push(0x6A00 + a.as_first_param() + b.as_second_param());
            }
            Instruction::NEG(a) => {
                output.push(0x6F00 + a.as_second_param());
            }
            Instruction::NOT(a) => {
                output.push(0x6F10 + a.as_second_param());
            }
            Instruction::INCB(a) => {
                output.push(0x6F20 + a.as_second_param());
            }
            Instruction::INCW(a) => {
                output.push(0x6F30 + a.as_second_param());
            }
            Instruction::DECB(a) => {
                output.push(0x6F40 + a.as_second_param());
            }
            Instruction::DECW(a) => {
                output.push(0x6F50 + a.as_second_param());
            }

            Instruction::IMM(v) => {
                output.push(v);
            }
        }
    }

    output
}
