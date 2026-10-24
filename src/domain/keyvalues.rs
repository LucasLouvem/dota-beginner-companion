//! Parser mínimo do formato Valve KeyValues (VDF), usado por
//! `libraryfolders.vdf` da Steam e pelos arquivos `.cfg` do GSI.

use thiserror::Error;

/// Valor KeyValues: texto ou seção com pares chave/valor em ordem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyValue {
    Text(String),
    Section(Vec<(String, KeyValue)>),
}

impl KeyValue {
    /// Primeiro filho com a chave informada (comparação sem diferenciar maiúsculas,
    /// como a própria Valve faz).
    pub fn get(&self, key: &str) -> Option<&KeyValue> {
        match self {
            KeyValue::Section(entries) => entries
                .iter()
                .find(|(entry_key, _)| entry_key.eq_ignore_ascii_case(key))
                .map(|(_, value)| value),
            KeyValue::Text(_) => None,
        }
    }

    /// Conteúdo, se o valor for texto.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            KeyValue::Text(text) => Some(text),
            KeyValue::Section(_) => None,
        }
    }

    /// Pares da seção; vazio se o valor for texto.
    pub fn entries(&self) -> &[(String, KeyValue)] {
        match self {
            KeyValue::Section(entries) => entries,
            KeyValue::Text(_) => &[],
        }
    }
}

/// Erro de sintaxe ao ler um documento KeyValues.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum KeyValuesError {
    #[error("texto entre aspas não terminado na linha {0}")]
    UnterminatedString(usize),
    #[error("'}}' inesperado na linha {0}")]
    UnexpectedClose(usize),
    #[error("chave sem valor na linha {0}")]
    MissingValue(usize),
    #[error("seção não fechada")]
    UnclosedSection,
}

#[derive(Debug, PartialEq)]
enum Token {
    Text(String),
    Open,
    Close,
}

fn tokenize(input: &str) -> Result<Vec<(Token, usize)>, KeyValuesError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    let mut line = 1;

    while let Some(current) = chars.next() {
        match current {
            '\n' => line += 1,
            whitespace if whitespace.is_whitespace() => {}
            '{' => tokens.push((Token::Open, line)),
            '}' => tokens.push((Token::Close, line)),
            '/' if chars.peek() == Some(&'/') => {
                for comment_char in chars.by_ref() {
                    if comment_char == '\n' {
                        line += 1;
                        break;
                    }
                }
            }
            '"' => {
                let start_line = line;
                let mut text = String::new();
                loop {
                    match chars.next() {
                        None => return Err(KeyValuesError::UnterminatedString(start_line)),
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some('n') => text.push('\n'),
                            Some('t') => text.push('\t'),
                            Some(escaped) => text.push(escaped),
                            None => return Err(KeyValuesError::UnterminatedString(start_line)),
                        },
                        Some(quoted_char) => {
                            if quoted_char == '\n' {
                                line += 1;
                            }
                            text.push(quoted_char);
                        }
                    }
                }
                tokens.push((Token::Text(text), start_line));
            }
            first_char => {
                let mut text = String::from(first_char);
                while let Some(&next_char) = chars.peek() {
                    if next_char.is_whitespace() || matches!(next_char, '{' | '}' | '"') {
                        break;
                    }
                    text.push(next_char);
                    chars.next();
                }
                tokens.push((Token::Text(text), line));
            }
        }
    }
    Ok(tokens)
}

/// Analisa um documento KeyValues e devolve a seção raiz implícita
/// (normalmente com um único filho, como `"libraryfolders" { ... }`).
pub fn parse(input: &str) -> Result<KeyValue, KeyValuesError> {
    let mut tokens = tokenize(input)?.into_iter();
    let root = parse_section(&mut tokens, false)?;
    Ok(KeyValue::Section(root))
}

fn parse_section(
    tokens: &mut impl Iterator<Item = (Token, usize)>,
    nested: bool,
) -> Result<Vec<(String, KeyValue)>, KeyValuesError> {
    let mut entries = Vec::new();
    loop {
        match tokens.next() {
            None if nested => return Err(KeyValuesError::UnclosedSection),
            None => return Ok(entries),
            Some((Token::Close, _)) if nested => return Ok(entries),
            Some((Token::Close, line)) => return Err(KeyValuesError::UnexpectedClose(line)),
            Some((Token::Open, line)) => return Err(KeyValuesError::MissingValue(line)),
            Some((Token::Text(key), line)) => match tokens.next() {
                Some((Token::Text(value), _)) => entries.push((key, KeyValue::Text(value))),
                Some((Token::Open, _)) => {
                    let child = parse_section(tokens, true)?;
                    entries.push((key, KeyValue::Section(child)));
                }
                Some((Token::Close, _)) | None => {
                    return Err(KeyValuesError::MissingValue(line));
                }
            },
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/domain/keyvalues_tests.rs"]
mod tests;
