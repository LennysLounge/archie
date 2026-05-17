#![allow(unused)]

use archie::isa::Register;
use clap::Parser;
use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::{self, Write},
    iter::Peekable,
    path::{Path, PathBuf},
};
use ux::u4;

use crate::token::{Token, TokenValue, tokenize};

mod token;

#[derive(Parser, Debug)]
struct Cli {
    #[arg(help = "The file to be assembled")]
    input_file: PathBuf,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let content = fs::read_to_string(&cli.input_file)?;

    let mut output: Vec<u16> = Vec::new();
    let mut labels: HashMap<&str, usize> = HashMap::new();
    let mut deferred_labels: HashMap<usize, &str> = HashMap::new();

    for (line_number, line) in content.lines().enumerate() {
        let mut token_list = match tokenize(line) {
            Ok(l) => l,
            Err(msg) => {
                println!("ERROR lien {line_number}: {msg}");
                println!("-> {line}");
                return Ok(());
            }
        };

        let mut token = token_list.iter().peekable();

        parse_line(&mut token, &mut output, &mut labels, &mut deferred_labels);

        // let Some(op) = token.next() else {
        //     continue;
        // };

        // let is_label = token.peek().is_some_and(|t| *t == ":");
        // if is_label {
        //     token.next();
        //     if let Err(msg) = expect_no_more_tokens(&mut token) {
        //         println!("ERROR line {line_number}: {msg}");
        //         println!("-> {line}");
        //         return Ok(());
        //     }
        //     labels.insert(op, output.len());
        // } else {
        //     match parse_instruction(op, &mut token, &mut output, &labels, &mut deferred_labels) {
        //         Err(msg) => {
        //             println!("ERROR line {line_number}: {msg}");
        //             println!("-> {line}");
        //             return Ok(());
        //         }
        //         Ok(_) => (),
        //     }
        // }
    }
    for (pos, label) in deferred_labels.iter() {
        if let Some(addr) = labels.get(label) {
            let imm16 = output
                .get_mut(*pos)
                .expect("The position that needs to be filled by the address must exist");
            *imm16 = (*addr * 2) as u16;
        } else {
            println!("ERROR: Unknown label '{label}'");
            return Ok(());
        }
    }

    let mut output_file = cli.input_file.clone();
    output_file.set_extension("bin");

    let mut out_file = File::create(output_file)?;
    for inst in output {
        out_file.write_all(&inst.to_le_bytes());
    }

    Ok(())
}

fn parse_line<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>> + Clone>,
    output: &mut Vec<u16>,
    labels: &mut HashMap<&'l str, usize>,
    deferred_labels: &mut HashMap<usize, &'l str>,
) -> Result<(), String> {
    let Some(t) = token.next() else {
        return Ok(());
    };
    let TokenValue::Text(token_text) = t.value else {
        return Err(format!(
            "Invalid start to line. Expected an instruction or a label definition."
        ));
    };

    let is_label = token
        .peek()
        .is_some_and(|t| matches!(t.value, TokenValue::Symbol(":")));
    if is_label {
        token.next();
        expect_no_more_tokens(token)?;
        labels.insert(token_text, output.len());
        return Ok(());
    }

    match token_text {
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
            let base_op = match token_text {
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
            let base_op = match token_text {
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
            RegisterOrLabel::Label(label) => {
                let target_addr = labels.get(label);
                if let Some(addr) = target_addr {
                    let offset = *addr as isize - output.len() as isize - 1;
                    if offset >= -128 {
                        output.push(0x5200 + (offset as u16 & 0x00FF));
                    } else {
                        output.push(0x5000);
                        output.push((*addr * 2) as u16);
                    }
                } else {
                    output.push(0x5000);
                    output.push(0u16);
                    deferred_labels.insert(output.len() - 1, label);
                }
            }
        },
        "JE" | "JNE" | "JL" | "JLE" | "JG" | "JGE" | "JB" | "JBE" | "JA" | "JAE" => {
            let (inst, inverse) = match token_text {
                "JE" => (0x5300, 0x5400),
                "JNE" => (0x5400, 0x5300),
                "JL" => (0x5500, 0x5800),
                "JLE" => (0x5600, 0x5700),
                "JG" => (0x5700, 0x5600),
                "JGE" => (0x5800, 0x5500),
                "JB" => (0x5900, 0x5C00),
                "JBE" => (0x5A00, 0x5B00),
                "JA" => (0x5B00, 0x5A00),
                "JAE" => (0x5C00, 0x5900),
                _ => unreachable!(),
            };
            let label = parse_label_terminating(token)?;
            let target_addr = labels.get(label);
            if let Some(addr) = target_addr {
                let offset = *addr as isize - output.len() as isize - 1;
                if offset >= -128 {
                    output.push(inst + (offset as u16 & 0x00FF));
                } else {
                    output.push(inverse + 0x0001);
                    output.push(0x5000);
                    output.push((*addr * 2) as u16);
                }
            } else {
                output.push(inverse + 0x0001);
                output.push(0x5000);
                output.push(0u16);
                deferred_labels.insert(output.len() - 1, label);
            }
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
            return Err(format!("Unknown instruction '{token_text}'"));
        }
    }
    expect_no_more_tokens(token)?;
    return Ok(());
}

fn parse_one_register<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>> + Clone>,
) -> Result<u4, String> {
    let register = parse_register(token)?;
    expect_no_more_tokens(token)?;
    Ok(register)
}

fn parse_two_registers<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>>>,
) -> Result<(u4, u4), String> {
    let dst = parse_register(token)?;
    parse_symbol(token, ",")?;
    let src = parse_register(token)?;
    expect_no_more_tokens(token)?;
    Ok((dst, src))
}

fn parse_register<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>>>,
) -> Result<u4, String> {
    let text = match token.next() {
        Some(Token {
            value: TokenValue::Text(text),
            ..
        }) => text,
        Some(Token { original, .. }) => {
            return Err(format!(
                "Expected a register name but got '{original}' instead"
            ));
        }
        None => {
            return Err(format!("Expected a register name but reached end of input"));
        }
    };
    let register = match *text {
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
        _ => return Err(format!("Invalid register '{text}'")),
    };
    Ok(register)
}

fn parse_symbol<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>>>,
    symbol: &str,
) -> Result<(), String> {
    match token.next() {
        Some(Token {
            value: TokenValue::Symbol(s),
            ..
        }) if *s == symbol => return Ok(()),
        Some(Token { original, .. }) => {
            return Err(format!(
                "Expected symbol '{symbol}' but got '{original}' instead"
            ));
        }
        None => {
            return Err(format!(
                "Expected symbol '{symbol}' but reached end of input"
            ));
        }
    };
}

fn parse_interrupt<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>>>,
) -> Result<u4, String> {
    let number = match token.next() {
        Some(Token {
            value: TokenValue::Number(n),
            ..
        }) => n,
        Some(Token { original, .. }) => {
            return Err(format!(
                "Expected an interrupt number but got '{original}' instead"
            ));
        }
        None => {
            return Err(format!(
                "Expected an interrupt number but reached end of input"
            ));
        }
    };
    let Ok(u4_number) = u4::try_from(*number as u64) else {
        return Err(format!("Invalid interrupt number '{number}'"));
    };
    expect_no_more_tokens(token)?;
    Ok(u4_number)
}

fn parse_register_u16<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>>>,
) -> Result<(u4, u16), String> {
    let dst = parse_register(token)?;
    parse_symbol(token, ",")?;
    let value = parse_u16(token)?;
    expect_no_more_tokens(token)?;
    Ok((dst, value))
}

fn parse_u16<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>>>,
) -> Result<u16, String> {
    let value = match token.next() {
        Some(Token {
            value: TokenValue::Number(n),
            ..
        }) => *n,
        Some(Token { original, .. }) => {
            return Err(format!("Expected a number but got '{original}' instead"));
        }
        None => return Err("Expected a number but reached end of input".to_owned()),
    };
    if value < i16::MIN as i64 || value > u16::MAX as i64 {
        return Err(format!(
            "Value '{value}' does not fit in the 16 bit value range from {} to {}",
            i16::MIN,
            u16::MAX
        ));
    }
    Ok(value as u16)
}

enum RegisterOrU16 {
    Register(u4),
    U16(u16),
}
fn parse_register_or_u16<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>> + Clone>,
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

fn expect_no_more_tokens<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>>>,
) -> Result<(), String> {
    if let Some(remaining) = token.next() {
        return Err(format!("Unexpected extra token '{}'", remaining.original));
    }
    Ok(())
}

enum RegisterOrLabel<'a> {
    Register(u4),
    Label(&'a str),
}

fn parse_register_or_label<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>> + Clone>,
) -> Result<RegisterOrLabel<'l>, String> {
    let mut attempt = token.clone();
    if let Ok(reg) = parse_register(&mut attempt) {
        *token = attempt;
        expect_no_more_tokens(token)?;
        return Ok(RegisterOrLabel::Register(reg));
    };
    let mut attempt = token.clone();
    if let Ok(label) = parse_label(&mut attempt) {
        *token = attempt;
        expect_no_more_tokens(token)?;
        return Ok(RegisterOrLabel::Label(label));
    };
    println!("next token: {:?}", token.peek());
    Err(format!(
        "Expected either a register or a label but got neither"
    ))
}

fn parse_label<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>>>,
) -> Result<&'l str, String> {
    let label = match token.next() {
        Some(Token {
            value: TokenValue::Text(label),
            ..
        }) => label,
        Some(Token { original, .. }) => {
            return Err(format!("Expected a label but got '{original}' instead"));
        }
        None => return Err(format!("Expected a label but reached end of input")),
    };
    Ok(label)
}

fn parse_label_terminating<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>>>,
) -> Result<&'l str, String> {
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

fn parse_register_and_address<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>>>,
) -> Result<(u4, AddressMode), String> {
    let dst = parse_register(token)?;
    parse_symbol(token, ",")?;
    let addr = parse_address(token)?;
    expect_no_more_tokens(token)?;
    Ok((dst, addr))
}

fn parse_address_and_register<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>>>,
) -> Result<(AddressMode, u4), String> {
    let addr = parse_address(token)?;
    parse_symbol(token, ",")?;
    let src = parse_register(token)?;
    expect_no_more_tokens(token)?;
    Ok((addr, src))
}

fn parse_address<'i, 'l: 'i>(
    token: &mut Peekable<impl Iterator<Item = &'i Token<'l>>>,
) -> Result<AddressMode, String> {
    parse_symbol(token, "[")?;

    match token.peek() {
        Some(Token {
            value: TokenValue::Symbol("--"),
            ..
        }) => {
            parse_symbol(token, "--")?;
            let reg = parse_register(token)?;
            parse_symbol(token, "]")?;
            return Ok(AddressMode::PreDecrement { r: reg });
        }
        _ => (),
    };

    let reg = parse_register(token)?;

    match token.peek() {
        Some(Token {
            value: TokenValue::Symbol("++"),
            ..
        }) => {
            parse_symbol(token, "++")?;
            parse_symbol(token, "]")?;
            return Ok(AddressMode::PostIncrement { r: reg });
        }
        Some(Token {
            value: TokenValue::Symbol("+"),
            ..
        }) => {
            parse_symbol(token, "+")?;
            let offset = parse_u16(token)?;
            parse_symbol(token, "]")?;
            return Ok(AddressMode::Offset { r: reg, offset });
        }
        Some(Token {
            value: TokenValue::Symbol("-"),
            ..
        }) => {
            parse_symbol(token, "-")?;
            let offset = parse_u16(token)?.wrapping_neg();
            parse_symbol(token, "]")?;
            return Ok(AddressMode::Offset { r: reg, offset });
        }
        _ => (),
    };
    parse_symbol(token, "]")?;
    Ok(AddressMode::Indirect { r: reg })
}
