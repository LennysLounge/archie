#![allow(unused)]

use archie::isa::Register;
use clap::Parser;
use std::{
    env,
    fs::{self, File},
    io::{self, Write},
    iter::Peekable,
};
use ux::u4;

mod ast;

#[derive(Parser, Debug)]
struct Cli {
    #[arg(help = "The file to be assembled")]
    input_file: String,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let content = fs::read_to_string(cli.input_file)?;

    let mut output: Vec<u16> = Vec::new();

    for (line_number, line) in content.lines().enumerate() {
        let token_list = tokenize(line);
        let mut token = token_list.iter().map(|x| *x).peekable();

        let Some(op) = token.next() else {
            continue;
        };

        let is_label = token.peek().is_some_and(|t| *t == ":");
        if is_label {
            token.next();
            if let Some(unused_token) = token.next() {
                println!("ERROR line {line_number}: Unexpected token '{unused_token}'");
                println!("-> {line}");
                return Ok(());
            }
            todo!("labels not implemented yet");
        } else {
            match parse_instruction(op, &mut token, &mut output) {
                Err(msg) => {
                    println!("ERROR line {line_number}: {msg}");
                    println!("-> {line}");
                }
                Ok(_) => (),
            }
        }
    }

    let mut out_file = File::create("out.bin")?;
    for inst in output {
        out_file.write_all(&inst.to_le_bytes());
    }

    Ok(())
}

fn parse_instruction<'a>(
    op: &str,
    token: &mut Peekable<impl Iterator<Item = &'a str> + Clone>,
    output: &mut Vec<u16>,
) -> Result<(), String> {
    match op {
        "NOP" => output.push(0x0000),
        "CALL" => match parse_register_or_u16(token)? {
            RegisterOrU16::Register(reg) => {
                output.push(0x0020 + u16::from(reg));
            }
            RegisterOrU16::U16(value) => {
                output.push(0x0010);
                output.push(value);
            }
        },
        "RET" => output.push(0x0030),
        "INT" => {
            let num = parse_interrupt(token)?;
            output.push(0x0040 + u16::from(num));
        }
        "IRET" => output.push(0x0050),
        "CLI" => output.push(0x0060),
        "STI" => output.push(0x0070),
        "DBG" => output.push(0x00BB),
        "HALT" => output.push(0x0FFF),
        "LDW" | "LDB" | "LDS" | "LRW" | "LRB" | "LRS" => {
            let base_op = match op {
                "LDW" => 0x1000,
                "LDB" => 0x1400,
                "LDS" => 0x1C00,
                "LRW" => 0x2000,
                "LRB" => 0x2400,
                "LRS" => 0x2C00,
                _ => unreachable!(),
            };
            let (dst, addr) = parse_register_and_address(token)?;
            output.push(base_op + (u16::from(dst) << 4) + addr.reg());
            if let Some(offset) = addr.offset() {
                output.push(offset);
            }
        }
        "STW" | "STB" => {
            let base_op = match op {
                "STW" => 0x3000,
                "STB" => 0x3400,
                _ => unreachable!(),
            };
            let (addr, src) = parse_address_and_register(token)?;
            output.push(base_op + (addr.reg() << 4) + u16::from(src));
            if let Some(offset) = addr.offset() {
                output.push(offset);
            }
        }
        "LDI" => {
            let (dst, value) = parse_register_u16(token)?;
            if value < 16 {
                output.push(0x4100 + (u16::from(dst) << 4) + value);
            } else if (value as i16) < 0 && (value as i16) > -16 {
                let v = value.wrapping_neg() & 0x000F;
                output.push(0x4200 + (u16::from(dst) << 4) + v);
            } else {
                output.push(0x4000 + (u16::from(dst) << 4));
                output.push(value);
            }
        }
        "MOV" => {
            let (dst, src) = parse_two_registers(token)?;
            output.push(0x4300 + (u16::from(dst) << 4) + u16::from(src));
        }
        "PUSH" => {
            let src = parse_one_register(token)?;
            output.push(0x4E00 + (u16::from(src) << 4));
        }
        "POP" => {
            let src = parse_one_register(token)?;
            output.push(0x4F00 + (u16::from(src) << 4));
        }
        "JMP" => match parse_register_or_label(token)? {
            RegisterOrLabel::Register(reg) => {
                output.push(0x5100 + (u16::from(reg) << 4));
            }
            RegisterOrLabel::Label(_) => todo!(),
        },
        "JE" | "JNE" | "JL" | "JLE" | "JG" | "JGE" | "JB" | "JBE" | "JA" | "JAE" => {
            let label = parse_label_terminating(token)?;
            todo!();
        }
        "ADD" => {
            let (dst, src) = parse_two_registers(token)?;
            output.push(0x6100 + (u16::from(dst) << 4) + u16::from(src));
        }
        "ADDC" => {
            let (dst, src) = parse_two_registers(token)?;
            output.push(0x6100 + (u16::from(dst) << 4) + u16::from(src));
        }
        "SUB" => {
            let (dst, src) = parse_two_registers(token)?;
            output.push(0x6200 + (u16::from(dst) << 4) + u16::from(src));
        }
        "SUBC" => {
            let (dst, src) = parse_two_registers(token)?;
            output.push(0x6300 + (u16::from(dst) << 4) + u16::from(src));
        }
        "AND" => {
            let (dst, src) = parse_two_registers(token)?;
            output.push(0x6400 + (u16::from(dst) << 4) + u16::from(src));
        }
        "OR" => {
            let (dst, src) = parse_two_registers(token)?;
            output.push(0x6500 + (u16::from(dst) << 4) + u16::from(src));
        }
        "XOR" => {
            let (dst, src) = parse_two_registers(token)?;
            output.push(0x6600 + (u16::from(dst) << 4) + u16::from(src));
        }
        "SHL" => {
            let (dst, src) = parse_two_registers(token)?;
            output.push(0x6700 + (u16::from(dst) << 4) + u16::from(src));
        }
        "SHR" => {
            let (dst, src) = parse_two_registers(token)?;
            output.push(0x6800 + (u16::from(dst) << 4) + u16::from(src));
        }
        "ASR" => {
            let (dst, src) = parse_two_registers(token)?;
            output.push(0x6900 + (u16::from(dst) << 4) + u16::from(src));
        }
        "CMP" => {
            let (dst, src) = parse_two_registers(token)?;
            output.push(0x6A00 + (u16::from(dst) << 4) + u16::from(src));
        }
        "NEG" => {
            let src = parse_one_register(token)?;
            output.push(0x6F00 + u16::from(src));
        }
        "NOT" => {
            let src = parse_one_register(token)?;
            output.push(0x6F10 + u16::from(src));
        }
        "INCB" => {
            let src = parse_one_register(token)?;
            output.push(0x6F20 + u16::from(src));
        }
        "INCW" => {
            let src = parse_one_register(token)?;
            output.push(0x6F30 + u16::from(src));
        }
        "DECB" => {
            let src = parse_one_register(token)?;
            output.push(0x6F40 + u16::from(src));
        }
        "DECW" => {
            let src = parse_one_register(token)?;
            output.push(0x6F50 + u16::from(src));
        }
        _ => {
            return Err(format!("Unknown instruction '{op}'"));
        }
    }
    Ok(())
}

fn tokenize(line: &str) -> Vec<&str> {
    let mut token = Vec::new();
    let mut current_token_start = None;
    for (i, char) in line.char_indices() {
        if char == ';' {
            break;
        }
        if let Some(token_start) = current_token_start {
            if char.is_alphanumeric() {
                continue;
            } else {
                token.push(&line[token_start..i]);
                current_token_start = None;
            }
        }
        if char.is_whitespace() {
            continue;
        } else if char.is_alphanumeric() && current_token_start.is_none() {
            current_token_start = Some(i);
        } else {
            token.push(&line[i..=i]);
        }
    }
    if let Some(token_start) = current_token_start {
        token.push(&line[token_start..]);
    }
    token
}

fn parse_one_register<'a>(
    token: &mut Peekable<impl Iterator<Item = &'a str>>,
) -> Result<u4, String> {
    let register = parse_register(token)?;
    expect_no_more_tokens(token)?;
    Ok(register)
}

fn parse_two_registers<'a>(
    token: &mut Peekable<impl Iterator<Item = &'a str>>,
) -> Result<(u4, u4), String> {
    let dst = parse_register(token)?;
    parse_tag(token, ",")?;
    let src = parse_register(token)?;
    expect_no_more_tokens(token)?;
    Ok((dst, src))
}

fn parse_register<'a>(token: &mut Peekable<impl Iterator<Item = &'a str>>) -> Result<u4, String> {
    let Some(t) = token.next() else {
        return Err(format!("Expected a register but got nothing"));
    };
    let register = match t {
        "R0" | "r0" => u4::new(0),
        "R1" | "r1" => u4::new(1),
        "R2" | "r2" => u4::new(2),
        "R3" | "r3" => u4::new(3),
        "R4" | "r4" => u4::new(4),
        "R5" | "r5" => u4::new(5),
        "R6" | "r6" => u4::new(6),
        "R7" | "r7" => u4::new(7),
        "R8" | "r8" => u4::new(8),
        "R9" | "r9" => u4::new(9),
        "R10" | "r10" => u4::new(10),
        "R11" | "r11" => u4::new(11),
        "R12" | "r12" => u4::new(12),
        "R13" | "r13" | "ST" => u4::new(13),
        "R14" | "r14" | "SP" => u4::new(14),
        "R15" | "r15" | "PC" => u4::new(15),
        _ => return Err(format!("Invalid register '{t}'")),
    };
    Ok(register)
}

fn parse_tag<'a>(
    token: &mut Peekable<impl Iterator<Item = &'a str>>,
    tag: &str,
) -> Result<(), String> {
    let Some(t) = token.next() else {
        return Err(format!("Expected a '{tag}' but got nothing"));
    };
    if t != tag {
        return Err(format!("Expected a '{tag}' but got '{t}'"));
    }
    Ok(())
}

fn parse_interrupt<'a>(token: &mut Peekable<impl Iterator<Item = &'a str>>) -> Result<u4, String> {
    let Some(t) = token.next() else {
        return Err(format!("Expected an interrupt number but got nothing"));
    };
    let Ok(int_number) = u8::from_str_radix(t, 10)
        .or_else(|_| u8::from_str_radix(t, 16))
        .or_else(|_| u8::from_str_radix(t, 8))
    else {
        return Err(format!("Expected an interrupt number but got '{t}'"));
    };
    let Ok(u4_number) = u4::try_from(int_number) else {
        return Err(format!("Invalid interrupt number '{int_number}'"));
    };
    expect_no_more_tokens(token)?;
    Ok(u4_number)
}

fn parse_register_u16<'a>(
    token: &mut Peekable<impl Iterator<Item = &'a str>>,
) -> Result<(u4, u16), String> {
    let dst = parse_register(token)?;
    parse_tag(token, ",")?;
    let value = parse_u16(token)?;
    expect_no_more_tokens(token)?;
    Ok((dst, value))
}

fn parse_u16<'a>(token: &mut Peekable<impl Iterator<Item = &'a str>>) -> Result<u16, String> {
    let Some(t) = token.next() else {
        return Err(format!("Expected a number but got nothing"));
    };
    let Ok(int_number) = i32::from_str_radix(t, 10)
        .or_else(|_| i32::from_str_radix(t, 16))
        .or_else(|_| i32::from_str_radix(t, 8))
    else {
        return Err(format!("Expected a number but got '{t}'"));
    };
    if int_number < i16::MIN as i32 || int_number > u16::MAX as i32 {
        return Err(format!(
            "Value '{int_number}' does not fit in the 16 bit value range from {} to {}",
            i16::MIN,
            u16::MAX
        ));
    }
    Ok(int_number as u16)
}

enum RegisterOrU16 {
    Register(u4),
    U16(u16),
}
fn parse_register_or_u16<'a>(
    token: &mut Peekable<impl Iterator<Item = &'a str> + Clone>,
) -> Result<RegisterOrU16, String> {
    let mut attempt = token.clone();
    if let Ok(reg) = parse_register(token) {
        *token = attempt;
        expect_no_more_tokens(token)?;
        return Ok(RegisterOrU16::Register(reg));
    };
    let mut attempt = token.clone();
    if let Ok(value) = parse_u16(token) {
        *token = attempt;
        expect_no_more_tokens(token)?;
        return Ok(RegisterOrU16::U16(value));
    };
    Err(format!(
        "Expected either a register or a 16 bit value but got neither"
    ))
}

fn expect_no_more_tokens<'a>(
    token: &mut Peekable<impl Iterator<Item = &'a str>>,
) -> Result<(), String> {
    if let Some(remaining) = token.next() {
        return Err(format!("Unexpected extra token '{remaining}'"));
    }
    Ok(())
}

enum RegisterOrLabel<'a> {
    Register(u4),
    Label(&'a str),
}

fn parse_register_or_label<'a>(
    token: &mut Peekable<impl Iterator<Item = &'a str> + Clone>,
) -> Result<RegisterOrLabel<'a>, String> {
    let mut attempt = token.clone();
    if let Ok(reg) = parse_register(token) {
        *token = attempt;
        expect_no_more_tokens(token)?;
        return Ok(RegisterOrLabel::Register(reg));
    };
    let mut attempt = token.clone();
    if let Ok(label) = parse_label(token) {
        *token = attempt;
        expect_no_more_tokens(token)?;
        return Ok(RegisterOrLabel::Label(label));
    };
    Err(format!(
        "Expected either a register or a label bit value but got neither"
    ))
}

fn parse_label<'a>(
    token: &mut Peekable<impl Iterator<Item = &'a str> + Clone>,
) -> Result<&'a str, String> {
    let Some(label) = token.next() else {
        return Err(format!("Expected a label but got nothing"));
    };
    Ok(label)
}

fn parse_label_terminating<'a>(
    token: &mut Peekable<impl Iterator<Item = &'a str> + Clone>,
) -> Result<&'a str, String> {
    let label = parse_label(token)?;
    expect_no_more_tokens(token)?;
    Ok(label)
}

enum AddressMode {
    Indirect { r: u4 },
    PostIncrement { r: u4 },
    PreDecrement { r: u4 },
    Offset { r: u4, offset: u16 },
}
impl AddressMode {
    fn mode(&self) -> u16 {
        match self {
            AddressMode::Indirect { .. } => 0x0000,
            AddressMode::PostIncrement { .. } => 0x0100,
            AddressMode::PreDecrement { .. } => 0x0200,
            AddressMode::Offset { .. } => 0x0300,
        }
    }
    fn reg(&self) -> u16 {
        match self {
            AddressMode::Indirect { r } => u16::from(*r),
            AddressMode::PostIncrement { r } => u16::from(*r),
            AddressMode::PreDecrement { r } => u16::from(*r),
            AddressMode::Offset { r, .. } => u16::from(*r),
        }
    }
    fn offset(&self) -> Option<u16> {
        match self {
            AddressMode::Indirect { .. } => None,
            AddressMode::PostIncrement { .. } => None,
            AddressMode::PreDecrement { .. } => None,
            AddressMode::Offset { offset, .. } => Some(*offset),
        }
    }
}

fn parse_register_and_address<'a>(
    token: &mut Peekable<impl Iterator<Item = &'a str>>,
) -> Result<(u4, AddressMode), String> {
    let dst = parse_register(token)?;
    parse_tag(token, ",")?;
    let addr = parse_address(token)?;
    expect_no_more_tokens(token)?;
    Ok((dst, addr))
}

fn parse_address_and_register<'a>(
    token: &mut Peekable<impl Iterator<Item = &'a str>>,
) -> Result<(AddressMode, u4), String> {
    let addr = parse_address(token)?;
    parse_tag(token, ",")?;
    let src = parse_register(token)?;
    expect_no_more_tokens(token)?;
    Ok((addr, src))
}

fn parse_address<'a>(
    token: &mut Peekable<impl Iterator<Item = &'a str>>,
) -> Result<AddressMode, String> {
    parse_tag(token, "[")?;

    let Some(next) = token.peek() else {
        return Err("Expected an addressing mode but got nothing".to_owned());
    };

    if *next == "-" {
        parse_tag(token, "-")?;
        parse_tag(token, "-")?;
        let reg = parse_register(token)?;
        parse_tag(token, "]")?;
        return Ok(AddressMode::PreDecrement { r: reg });
    }
    let reg = parse_register(token)?;
    let Some(next) = token.peek() else {
        return Err("Expected an addressing mode but got nothing".to_owned());
    };
    if *next == "]" {
        parse_tag(token, "]")?;
        return Ok(AddressMode::Indirect { r: reg });
    }
    if *next != "+" {
        return Err("Expected either a post increment or a offset addition".to_owned());
    }
    parse_tag(token, "+")?;

    let Some(next) = token.peek() else {
        return Err("Expected either a post increment or a offset addition".to_owned());
    };

    if *next == "+" {
        parse_tag(token, "]")?;
        return Ok(AddressMode::PostIncrement { r: reg });
    }

    let offset = parse_u16(token)?;
    parse_tag(token, "]")?;

    Ok(AddressMode::Offset { r: reg, offset })
}
