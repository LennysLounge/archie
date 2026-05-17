use std::iter::Peekable;

#[derive(Debug)]
pub struct Token<'a> {
    pub value: TokenValue<'a>,
    pub original: &'a str,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug)]
pub enum TokenValue<'a> {
    Text(&'a str),
    Number(i64),
    Symbol(&'a str),
}

pub fn tokenize<'a>(line: &'a str) -> Result<Vec<Token<'a>>, String> {
    let mut token = Vec::new();
    let mut chars = line.char_indices().peekable();
    while let Some((start_idx, next_char)) = chars.next() {
        if next_char.is_whitespace() {
            continue;
        }
        match next_char {
            'a'..='z' | 'A'..='Z' | '_' => token.push(lex_identifier(line, start_idx, &mut chars)),
            '1'..='9' => token.push(lex_number_base_10(line, start_idx, &mut chars)?),
            '0' => token.push(lex_number_with_any_base(line, start_idx, &mut chars)?),
            '+' if chars.peek().is_some_and(|(_, c)| *c == '+') => {
                let (end_idx, _) = chars.next().unwrap();
                token.push(Token {
                    value: TokenValue::Symbol(&line[start_idx..=end_idx]),
                    original: &line[start_idx..=end_idx],
                    start: start_idx,
                    end: end_idx,
                });
            }
            '-' if chars.peek().is_some_and(|(_, c)| *c == '-') => {
                let (end_idx, _) = chars.next().unwrap();
                token.push(Token {
                    value: TokenValue::Symbol(&line[start_idx..=end_idx]),
                    original: &line[start_idx..=end_idx],
                    start: start_idx,
                    end: end_idx,
                });
            }
            ';' => break,
            _ => token.push(Token {
                value: TokenValue::Symbol(&line[start_idx..start_idx + 1]),
                original: &line[start_idx..start_idx + 1],
                start: start_idx,
                end: start_idx,
            }),
        }
    }
    Ok(token)
}

fn lex_identifier<'a>(
    line: &'a str,
    start: usize,
    chars: &mut Peekable<impl Iterator<Item = (usize, char)>>,
) -> Token<'a> {
    let mut end = start;
    while let Some((idx, char)) = chars.peek() {
        if matches!(char, 'a'..='z' | 'A'..='Z' | '_' | '0'..='9') {
            end = *idx;
            chars.next();
        } else {
            break;
        }
    }
    Token {
        value: TokenValue::Text(&line[start..=end]),
        original: &line[start..=end],
        start,
        end,
    }
}

fn lex_number_base_10<'a>(
    line: &'a str,
    start: usize,
    chars: &mut Peekable<impl Iterator<Item = (usize, char)>>,
) -> Result<Token<'a>, String> {
    let mut end = start;
    while let Some((idx, char)) = chars.peek() {
        match char {
            '_' | '0'..='9' => {
                end = *idx;
                chars.next();
            }
            'a'..='z' | 'A'..='Z' => {
                let msg = format!("Invalid digit '{char}' in decimal literal");
                chars.next();
                return Err(msg);
            }
            _ => break,
        }
    }

    let original = &line[start..=end];
    let sanitized = original.replace("_", "");
    let num =
        i64::from_str_radix(&sanitized, 10).expect("Input should only contain valid characters");
    Ok(Token {
        value: TokenValue::Number(num),
        original,
        start,
        end,
    })
}

fn lex_number_with_any_base<'a>(
    line: &'a str,
    start: usize,
    chars: &mut Peekable<impl Iterator<Item = (usize, char)>>,
) -> Result<Token<'a>, String> {
    match chars.next() {
        Some((_, '0'..='9')) => lex_number_base_10(line, start, chars),
        Some((_, 'x')) => lex_number_base_16(line, start, chars),
        Some((_, 'b')) => lex_number_base_2(line, start, chars),
        Some((_, c)) => Err(format!("Invalid character '{c}' in number literal")),
        None => Ok(Token {
            value: TokenValue::Number(0),
            original: &line[start..=start],
            start,
            end: start,
        }),
    }
}

fn lex_number_base_16<'a>(
    line: &'a str,
    start: usize,
    chars: &mut Peekable<impl Iterator<Item = (usize, char)>>,
) -> Result<Token<'a>, String> {
    let mut end = start;
    while let Some((idx, char)) = chars.peek() {
        match char {
            '_' | '0'..='9' | 'a'..='f' | 'A'..='F' => {
                end = *idx;
                chars.next();
            }
            'g'..='z' | 'G'..='Z' => {
                let msg = format!("Invalid digit '{char}' in hex literal");
                chars.next();
                return Err(msg);
            }
            _ => break,
        }
    }
    if start == end {
        return Err(format!("Incomplete binary litteral"));
    }

    let original = &line[start..=end];
    let sanitized = original[2..].replace("_", "");
    let num =
        i64::from_str_radix(&sanitized, 16).expect("Input should only contain valid characters");
    Ok(Token {
        value: TokenValue::Number(num),
        original,
        start,
        end,
    })
}

fn lex_number_base_2<'a>(
    line: &'a str,
    start: usize,
    chars: &mut Peekable<impl Iterator<Item = (usize, char)>>,
) -> Result<Token<'a>, String> {
    let mut end = start;
    while let Some((idx, char)) = chars.peek() {
        match char {
            '_' | '0'..='1' => {
                end = *idx;
                chars.next();
            }
            '2'..='9' | 'a'..='z' | 'A'..='Z' => {
                let msg = format!("Invalid digit '{char}' in binary literal");
                chars.next();
                return Err(msg);
            }
            _ => break,
        }
    }
    if start == end {
        return Err(format!("Incomplete binary litteral"));
    }

    let original = &line[start..=end];
    let sanitized = original[2..].replace("_", "");
    let num =
        i64::from_str_radix(&sanitized, 2).expect("Input should only contain valid characters");
    Ok(Token {
        value: TokenValue::Number(num),
        original,
        start,
        end,
    })
}
