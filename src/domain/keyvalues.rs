//! Parser mínimo do formato Valve KeyValues (VDF), usado por
//! `libraryfolders.vdf` da Steam e pelos arquivos `.cfg` do GSI.

use thiserror::Error;

/// Valor KeyValues: texto ou seção com pares chave/valor em ordem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KvValue {
    Text(String),
    Section(Vec<(String, KvValue)>),
}

impl KvValue {
    /// Primeiro filho com a chave informada (comparação sem diferenciar maiúsculas,
    /// como a própria Valve faz).
    pub fn get(&self, key: &str) -> Option<&KvValue> {
        match self {
            KvValue::Section(entries) => entries
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            KvValue::Text(_) => None,
        }
    }

    /// Conteúdo, se o valor for texto.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            KvValue::Text(s) => Some(s),
            KvValue::Section(_) => None,
        }
    }

    /// Pares da seção; vazio se o valor for texto.
    pub fn entries(&self) -> &[(String, KvValue)] {
        match self {
            KvValue::Section(entries) => entries,
            KvValue::Text(_) => &[],
        }
    }
}

/// Erro de sintaxe ao ler um documento KeyValues.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum KvError {
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
    Str(String),
    Open,
    Close,
}

fn tokenize(input: &str) -> Result<Vec<(Token, usize)>, KvError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    let mut line = 1;

    while let Some(c) = chars.next() {
        match c {
            '\n' => line += 1,
            c if c.is_whitespace() => {}
            '{' => tokens.push((Token::Open, line)),
            '}' => tokens.push((Token::Close, line)),
            '/' if chars.peek() == Some(&'/') => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        line += 1;
                        break;
                    }
                }
            }
            '"' => {
                let start = line;
                let mut s = String::new();
                loop {
                    match chars.next() {
                        None => return Err(KvError::UnterminatedString(start)),
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => s.push(other),
                            None => return Err(KvError::UnterminatedString(start)),
                        },
                        Some(c) => {
                            if c == '\n' {
                                line += 1;
                            }
                            s.push(c);
                        }
                    }
                }
                tokens.push((Token::Str(s), start));
            }
            c => {
                let mut s = String::from(c);
                while let Some(&n) = chars.peek() {
                    if n.is_whitespace() || n == '{' || n == '}' || n == '"' {
                        break;
                    }
                    s.push(n);
                    chars.next();
                }
                tokens.push((Token::Str(s), line));
            }
        }
    }
    Ok(tokens)
}

/// Analisa um documento KeyValues e devolve a seção raiz implícita
/// (normalmente com um único filho, como `"libraryfolders" { ... }`).
pub fn parse(input: &str) -> Result<KvValue, KvError> {
    let tokens = tokenize(input)?;
    let mut iter = tokens.into_iter();
    let root = parse_section(&mut iter, false)?;
    Ok(KvValue::Section(root))
}

fn parse_section(
    iter: &mut impl Iterator<Item = (Token, usize)>,
    nested: bool,
) -> Result<Vec<(String, KvValue)>, KvError> {
    let mut entries = Vec::new();
    loop {
        match iter.next() {
            None if nested => return Err(KvError::UnclosedSection),
            None => return Ok(entries),
            Some((Token::Close, _)) if nested => return Ok(entries),
            Some((Token::Close, line)) => return Err(KvError::UnexpectedClose(line)),
            Some((Token::Open, line)) => return Err(KvError::MissingValue(line)),
            Some((Token::Str(key), line)) => match iter.next() {
                Some((Token::Str(value), _)) => entries.push((key, KvValue::Text(value))),
                Some((Token::Open, _)) => {
                    let child = parse_section(iter, true)?;
                    entries.push((key, KvValue::Section(child)));
                }
                Some((Token::Close, _)) | None => return Err(KvError::MissingValue(line)),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_sections_and_comments() {
        let doc = r#"
            // comentário
            "root"
            {
                "a"   "1"
                "sub" { "b" "two words" }
                unquoted value
            }
        "#;
        let kv = parse(doc).unwrap();
        let root = kv.get("root").unwrap();
        assert_eq!(root.get("a").unwrap().as_text(), Some("1"));
        assert_eq!(
            root.get("SUB").unwrap().get("b").unwrap().as_text(),
            Some("two words")
        );
        assert_eq!(root.get("unquoted").unwrap().as_text(), Some("value"));
    }

    #[test]
    fn handles_escapes() {
        let kv = parse(r#""p" "C:\\Games\\\"x\"""#).unwrap();
        assert_eq!(kv.get("p").unwrap().as_text(), Some(r#"C:\Games\"x""#));
    }

    #[test]
    fn reports_errors() {
        assert_eq!(parse(r#""a" { "b" "c""#), Err(KvError::UnclosedSection));
        assert_eq!(parse(r#""a" "b" }"#), Err(KvError::UnexpectedClose(1)));
        assert_eq!(parse(r#""a""#), Err(KvError::MissingValue(1)));
        assert_eq!(parse("\"a"), Err(KvError::UnterminatedString(1)));
    }
}
