use num_enum::TryFromPrimitive;

pub enum InstructionFormat {
    Misc(Misc),
    Memory(Memory),
    Alu(Alu),
    If(If),
    Offset(Offset),
    Error(u16),
}
impl InstructionFormat {
    pub fn cycles(&self) -> i32{
        match self{
            InstructionFormat::Misc(misc) => misc.cycles(),
            InstructionFormat::Memory(memory) => memory.cycles(),
            InstructionFormat::Alu(alu) => alu.cycles(),
            InstructionFormat::If(iff) => iff.cycles(),
            InstructionFormat::Offset(offset) => offset.cycles(),
            InstructionFormat::Error(_) => 1,
        }
    }
    pub fn has_immediate_value(&self) -> bool {
        match self {
            InstructionFormat::Misc(misc) => misc.has_immediate_value(),
            InstructionFormat::Memory(memory) => memory.has_immedaite_value(),
            InstructionFormat::Alu(alu) => alu.has_immediate_value(),
            InstructionFormat::If(iff) => iff.has_immediate_value(),
            InstructionFormat::Offset(offset) => offset.has_immediate_value(),
            InstructionFormat::Error(_) => false,
        }
    }
    pub fn resets_skip_flag(&self) -> bool{
        match self {
            InstructionFormat::If(_) => false,
            _ => true,
        }
    }
    pub fn trap() -> Self {
        InstructionFormat::Misc(Misc {
            opcode: MiscOpcode::TRAP,
            p1: 0,
            p2: 0,
        })
    }
}

pub struct Misc {
    pub opcode: MiscOpcode,
    pub p1: u8,
    pub p2: u8,
}
impl Misc {
    pub fn has_immediate_value(&self) -> bool {
        match self.opcode {
            MiscOpcode::CALLI => true,
            _ => false,
        }
    }
    pub fn cycles(&self) -> i32 {
        self.opcode.cycles()
    }
}
#[derive(TryFromPrimitive)]
#[repr(u16)]
pub enum MiscOpcode {
    TRAP = 0x0000,
    CLI = 0x0100,
    STI = 0x0200,
    IRET = 0x0300,
    RET = 0x0400,
    CALLI = 0x0500,
    DBG = 0x0600,
    HALT = 0x0700,
    INT = 0x0800,
    CALLR = 0x0900,
    NEG = 0x0A00,
}
impl MiscOpcode{
    pub fn cycles(&self) -> i32{
        1
    }
}
pub struct Memory {
    pub opcode: MemoryOpcode,
    pub address_mode: AddressMode,
    pub register: usize,
}
impl Memory {
    pub fn has_immedaite_value(&self) -> bool {
        match self.address_mode {
            AddressMode::Offset(_) => true,
            AddressMode::Absolute => true,
            _ => false,
        }
    }
    pub fn cycles(&self) -> i32 {
        self.opcode.cycles()
    }
}
#[derive(TryFromPrimitive)]
#[repr(u16)]
pub enum MemoryOpcode {
    LDX = 0x1000,
    LRX = 0x2000,
    STX = 0x3000,
}
impl MemoryOpcode{
    pub fn cycles(&self) -> i32{
        1
    }
}
pub struct Alu {
    pub opcode: AluOpcode,
    pub parameter: Parameter,
    pub register: usize,
}
impl Alu {
    pub fn has_immediate_value(&self) -> bool {
        match self.parameter {
            Parameter::Immediate => true,
            _ => false,
        }
    }
    pub fn cycles(&self) -> i32 {
        self.opcode.cycles()
    }
}
#[derive(TryFromPrimitive)]
#[repr(u16)]
pub enum AluOpcode {
    ADD = 0x4000,
    ADDC = 0x4400,
    SUB = 0x4800,
    SUBC = 0x4C00,
    MUL = 0x5000,
    MULS = 0x5400,
    DIV = 0x5800,
    DIVS = 0x5C00,
    SHL = 0x6000,
    SHLC = 0x6400,
    SHR = 0x6800,
    SHRC = 0x6C00,
    SHA = 0x7000,
    MOV = 0x7400,
    AND = 0x7800,
    OR = 0x7C00,
    XOR = 0x8000,
    BCL = 0x8400,
}
impl AluOpcode{
    pub fn cycles(&self) -> i32{
        1
    }
}
pub struct If {
    pub opcode: IfOpcode,
    pub parameter: Parameter,
    pub register: usize,
}
impl If {
    pub fn has_immediate_value(&self) -> bool {
        match self.parameter {
            Parameter::Immediate => true,
            _ => false,
        }
    }
    pub fn cycles(&self) -> i32 {
        self.opcode.cycles()
    }
}
#[derive(TryFromPrimitive)]
#[repr(u16)]
pub enum IfOpcode {
    IFE = 0x8800,
    IFNE = 0x8C00,
    IFL = 0x9000,
    IFLE = 0x9400,
    IFG = 0x9800,
    IFGE = 0x9C00,
    IFB = 0xA000,
    IFBE = 0xA400,
    IFA = 0xA800,
    IFAE = 0xAC00,
}
impl IfOpcode{
    pub fn cycles(&self) -> i32{
        1
    }
}
pub struct Offset {
    pub opcode: OffsetOpcode,
    pub offset: u16,
}
impl Offset {
    pub fn has_immediate_value(&self) -> bool {
        false
    }
    pub fn cycles(&self) -> i32 {
        self.opcode.cycles()
    }
}
#[derive(TryFromPrimitive)]
#[repr(u16)]
pub enum OffsetOpcode {
    JMP,
    CALL,
}
impl OffsetOpcode{
    pub fn cycles(&self) -> i32{
        1
    }
}

pub fn decode_instruction(inst: u16) -> InstructionFormat {
    match inst {
        0x0000..=0x0FFF => {
            let Ok(opcode) = MiscOpcode::try_from(inst) else {
                return InstructionFormat::Error(inst);
            };
            let p1 = (inst >> 4 & 0xF) as u8;
            let p2 = (inst & 0xF) as u8;
            InstructionFormat::Misc(Misc { opcode, p1, p2 })
        }
        0x1000..=0x3FFF => {
            let Ok(opcode) = MemoryOpcode::try_from(inst) else {
                return InstructionFormat::Error(inst);
            };
            let address_mode = decode_address_mode(inst);
            let register = (inst & 0x000F) as usize;
            InstructionFormat::Memory(Memory {
                opcode,
                address_mode,
                register,
            })
        }
        0x4000..=0x7FFF => {
            let Ok(opcode) = AluOpcode::try_from(inst) else {
                return InstructionFormat::Error(inst);
            };
            let parameter = decode_parameter(inst);
            let register = (inst & 0x000F) as usize;
            InstructionFormat::Alu(Alu {
                opcode,
                parameter,
                register,
            })
        }
        0x8000..=0xA7FF => {
            let Ok(opcode) = IfOpcode::try_from(inst) else {
                return InstructionFormat::Error(inst);
            };
            let parameter = decode_parameter(inst);
            let register = (inst & 0x000F) as usize;
            InstructionFormat::If(If {
                opcode,
                parameter,
                register,
            })
        }
        0xA800..=0xACFF => {
            let Ok(opcode) = OffsetOpcode::try_from(inst) else {
                return InstructionFormat::Error(inst);
            };
            let offset = inst & 0x03FF;
            InstructionFormat::Offset(Offset { opcode, offset })
        }
        _ => todo!(),
    }
}

pub enum Parameter {
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

pub enum AddressMode {
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
