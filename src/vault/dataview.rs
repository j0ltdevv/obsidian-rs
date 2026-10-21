//! Filtrage et tri des notes par champ de frontmatter (style « Dataview »).
//!
//! [`Query`] exprime une condition sur une propriété du frontmatter YAML d'une
//! note. [`filter_notes`] retourne les indices des notes qui satisfont **toutes**
//! les requêtes (conjonction). [`sort_by_field`] réordonne une liste d'indices
//! selon la valeur d'un champ.
//!
//! # Champ absent
//!
//! Une note qui ne possède pas le champ demandé (pas de frontmatter, frontmatter
//! qui n'est pas un mapping, ou clé absente) est :
//!
//! - exclue par [`filter_notes`] ;
//! - placée en **fin** de liste par [`sort_by_field`], quel que soit le sens du
//!   tri.
//!
//! # Valeurs numériques
//!
//! `Gt`, `Lt` et le tri tentent de lire la valeur en tant que `f64` : un nombre
//! YAML est lu directement, une chaîne est parsée. Toute autre forme (booléen,
//! séquence, mapping, `null`, chaîne non numérique) n'est pas numérique : la
//! comparaison échoue (la note est exclue) et le tri la traite comme du texte.
//!
//! # Langage de requête Dataview (DQL)
//!
//! [`query_notes`] et [`parse_dql`] ajoutent un sous-ensemble du langage
//! Dataview : `FROM #tag`, `WHERE ...`, `SORT ...`, `LIMIT ...`. Une requête
//! vide renvoie toutes les notes. Voir [`parse_dql`] pour la syntaxe complète.

use std::cmp::Ordering;
use std::fs;
use std::time::UNIX_EPOCH;

use serde_yaml::Value;

use super::note::Note;

/// Comparaison appliquée à la valeur d'un champ.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldCmp {
    /// Égalité, sur la forme texte de la valeur.
    Eq(String),
    /// La forme texte de la valeur contient la sous-chaîne.
    Contains(String),
    /// Strictement supérieur, valeur numérique.
    Gt(f64),
    /// Strictement inférieur, valeur numérique.
    Lt(f64),
}

/// Condition sur un champ du frontmatter d'une note.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// Nom du champ.
    pub field: String,
    /// Comparaison à appliquer à la valeur du champ.
    pub cmp: FieldCmp,
}

impl Query {
    /// Crée une condition sur `field`.
    pub fn new(field: impl Into<String>, cmp: FieldCmp) -> Self {
        Self {
            field: field.into(),
            cmp,
        }
    }
}

/// Retourne la valeur du champ `field` dans le frontmatter de `note`, ou `None`
/// si le champ est absent ou si le frontmatter n'est pas un mapping.
fn field_value<'a>(note: &'a Note, field: &str) -> Option<&'a Value> {
    note.metadata
        .frontmatter
        .as_ref()
        .and_then(Value::as_mapping)
        .and_then(|mapping| mapping.get(field))
}

/// Forme texte d'une valeur YAML, utilisée par `Eq` et `Contains`.
fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Null => String::new(),
        // Séquences et mappings : sérialisation YAML compacte, de sorte que le
        // contenu reste lisible (une liste `tags: [rust, test]` devient
        // « [rust, test] »).
        _ => serde_yaml::to_string(value)
            .unwrap_or_default()
            .trim()
            .to_string(),
    }
}

/// Tente de convertir une valeur YAML en `f64`.
fn as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

/// Compare deux valeurs présentes : numérique si les deux sont lisibles en
/// `f64`, texte sinon.
fn compare_values(left: &Value, right: &Value) -> Ordering {
    match (as_f64(left), as_f64(right)) {
        (Some(left), Some(right)) => left.partial_cmp(&right).unwrap_or(Ordering::Equal),
        _ => value_text(left).cmp(&value_text(right)),
    }
}

/// Vérifie que `note` satisfait `query`. Une note sans le champ ne le satisfait
/// jamais.
fn matches_query(note: &Note, query: &Query) -> bool {
    let Some(value) = field_value(note, &query.field) else {
        return false;
    };

    match &query.cmp {
        FieldCmp::Eq(expected) => value_text(value) == *expected,
        FieldCmp::Contains(needle) => value_text(value).contains(needle),
        FieldCmp::Gt(threshold) => as_f64(value).is_some_and(|v| v > *threshold),
        FieldCmp::Lt(threshold) => as_f64(value).is_some_and(|v| v < *threshold),
    }
}

/// Indices des notes qui satisfont **toutes** les requêtes (AND).
///
/// L'ordre d'origine est conservé. Une requête vide ne filtre rien : toutes les
/// notes sont retenues. Une note qui ne possède pas le champ d'une requête est
/// exclue.
pub fn filter_notes(notes: &[Note], queries: &[Query]) -> Vec<usize> {
    notes
        .iter()
        .enumerate()
        .filter(|(_, note)| queries.iter().all(|query| matches_query(note, query)))
        .map(|(index, _)| index)
        .collect()
}

/// Réordonne `indices` selon la valeur du champ `field`.
///
/// - `ascending = true` : valeurs croissantes, puis les notes sans champ.
/// - `ascending = false` : valeurs décroissantes, puis les notes sans champ.
///
/// Les notes qui partagent la même valeur conservent leur ordre relatif (tri
/// stable). Les indices hors de `notes` provoquent un `panic`.
pub fn sort_by_field(notes: &[Note], field: &str, ascending: bool, indices: &mut [usize]) {
    indices.sort_by(|&left, &right| {
        let left_value = field_value(&notes[left], field);
        let right_value = field_value(&notes[right], field);

        // Les notes sans le champ passent toujours en fin, quel que soit le
        // sens du tri.
        let presence = match (left_value.is_some(), right_value.is_some()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => Ordering::Equal,
        };
        if presence != Ordering::Equal {
            return presence;
        }

        let value_order = match (left_value, right_value) {
            (Some(left), Some(right)) => compare_values(left, right),
            _ => Ordering::Equal,
        };
        if value_order == Ordering::Equal {
            // Stabilité : à valeur égale, on restaure l'ordre des notes
            // (indices croissants), indépendamment du sens du tri.
            left.cmp(&right)
        } else if ascending {
            value_order
        } else {
            value_order.reverse()
        }
    });
}
// =============================================================================
// Langage de requête Dataview (sous-ensemble de DQL)
// =============================================================================
//
// Syntaxe supportée (clauses dans cet ordre) :
//   [FROM <source>] [WHERE <condition>] [SORT <champ> [ASC|DESC]] [LIMIT <n>]
//
//   - `FROM #tag`      : notes portant le tag (sous-tags inclus).
//   - `FROM "dossier"` : notes du dossier et de ses sous-dossiers.
//   - `FROM *` ou absente : toutes les notes.
//   - `WHERE <condition>` : AND/OR/NOT, parenthèses, comparaisons `=`, `!=`,
//     `<`, `>`, `<=`, `>=`, `contains`, `startswith`, `endswith`.
//   - `SORT <champ> [ASC|DESC]` : tri stable ; les notes sans valeur passent en
//     fin, quel que soit le sens.
//   - `LIMIT <n>` : ne garde que les `n` premières notes.
//
// Champs : `file.name`, `file.path`, `file.folder`, `file.ext`, `file.size`,
// `file.mtime`, `file.ctime`, `file.tags` (liste), `file.links` (liste), et
// toute propriété du frontmatter de la note.
//
// `file.size`, `file.mtime` et `file.ctime` sont lus sur le système de fichiers
// (`std::fs::metadata`). Pour une note synthétique dont le chemin n'existe pas
// sur disque (tests), ces champs sont considérés absents.

use std::fmt;

/// Erreur de parsing d'une requête Dataview, localisée (ligne/colonne).
#[derive(Debug, Clone, PartialEq)]
pub struct QueryError {
    message: String,
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for QueryError {}

impl QueryError {
    fn at(line: usize, column: usize, msg: impl Into<String>) -> Self {
        let msg = msg.into();
        Self {
            message: format!("{msg} (ligne {line}, colonne {column})"),
        }
    }
}

/// Source du `FROM`.
#[derive(Debug, Clone, PartialEq)]
enum Source {
    All,
    Tag(String),
    Folder(String),
}

/// Champ natif `file.*`.
#[derive(Debug, Clone, PartialEq)]
enum FileField {
    Name,
    Path,
    Folder,
    Size,
    Mtime,
    Ctime,
    Ext,
    Tags,
    Links,
}

/// Référence à un champ : natif `file.*` ou propriété du frontmatter.
#[derive(Debug, Clone, PartialEq)]
enum FieldRef {
    File(FileField),
    Property(String),
}

/// Opérateur de comparaison d'une condition `WHERE`.
#[derive(Debug, Clone, PartialEq)]
enum CmpOp {
    Eq,
    Neq,
    Lt,
    Gt,
    Leq,
    Geq,
    Contains,
    Startswith,
    Endswith,
}

/// Valeur littérale à droite d'un opérateur.
#[derive(Debug, Clone, PartialEq)]
enum Literal {
    Text(String),
    Number(f64),
}

/// Arbre d'expression d'une condition `WHERE`.
#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Cmp {
        field: FieldRef,
        op: CmpOp,
        value: Literal,
    },
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
}

/// Spécification `SORT`.
#[derive(Debug, Clone, PartialEq)]
struct SortSpec {
    field: FieldRef,
    ascending: bool,
}

/// Requête Dataview parsée.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedDql {
    source: Source,
    condition: Option<Expr>,
    sort: Option<SortSpec>,
    limit: Option<usize>,
}

/// Jeton lexé.
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Word(String),
    Quoted(String),
    Eq,
    Neq,
    Lt,
    Gt,
    Leq,
    Geq,
    LParen,
    RParen,
}

/// Jeton accompagné de sa position source.
#[derive(Debug, Clone)]
struct Spanned {
    token: Token,
    line: usize,
    column: usize,
}

/// Position (ligne, colonne) de la fin de `input`.
fn eof_position(input: &str) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for c in input.chars() {
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

/// Lexe une requête en jetons. Retourne une erreur localisée en cas de
/// guillemet non fermé ou de caractère invalide.
fn lex(input: &str) -> Result<Vec<Spanned>, QueryError> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    let mut line = 1;
    let mut column = 1;

    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\r' => {
                i += 1;
                column += 1;
            }
            '\n' => {
                i += 1;
                line += 1;
                column = 1;
            }
            '(' => {
                tokens.push(Spanned {
                    token: Token::LParen,
                    line,
                    column,
                });
                i += 1;
                column += 1;
            }
            ')' => {
                tokens.push(Spanned {
                    token: Token::RParen,
                    line,
                    column,
                });
                i += 1;
                column += 1;
            }
            '"' => {
                let start_line = line;
                let start_column = column;
                i += 1;
                column += 1;
                let mut text = String::new();
                let mut closed = false;
                while i < chars.len() {
                    let d = chars[i];
                    if d == '"' {
                        closed = true;
                        i += 1;
                        column += 1;
                        break;
                    }
                    if d == '\n' {
                        line += 1;
                        column = 1;
                    } else {
                        column += 1;
                    }
                    text.push(d);
                    i += 1;
                }
                if !closed {
                    return Err(QueryError::at(
                        start_line,
                        start_column,
                        "guillemet non fermé",
                    ));
                }
                tokens.push(Spanned {
                    token: Token::Quoted(text),
                    line: start_line,
                    column: start_column,
                });
            }
            '=' => {
                tokens.push(Spanned {
                    token: Token::Eq,
                    line,
                    column,
                });
                i += 1;
                column += 1;
            }
            '<' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Spanned {
                    token: Token::Leq,
                    line,
                    column,
                });
                i += 2;
                column += 2;
            }
            '<' => {
                tokens.push(Spanned {
                    token: Token::Lt,
                    line,
                    column,
                });
                i += 1;
                column += 1;
            }
            '>' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Spanned {
                    token: Token::Geq,
                    line,
                    column,
                });
                i += 2;
                column += 2;
            }
            '>' => {
                tokens.push(Spanned {
                    token: Token::Gt,
                    line,
                    column,
                });
                i += 1;
                column += 1;
            }
            '!' if i + 1 < chars.len() && chars[i + 1] == '=' => {
                tokens.push(Spanned {
                    token: Token::Neq,
                    line,
                    column,
                });
                i += 2;
                column += 2;
            }
            '!' => {
                return Err(QueryError::at(line, column, "caractère « ! » inattendu"));
            }
            _ => {
                let start_line = line;
                let start_column = column;
                let mut word = String::new();
                while i < chars.len() {
                    let d = chars[i];
                    if d.is_whitespace() || matches!(d, '(' | ')' | '"' | '=' | '<' | '>' | '!') {
                        break;
                    }
                    word.push(d);
                    i += 1;
                    if d == '\n' {
                        line += 1;
                        column = 1;
                    } else {
                        column += 1;
                    }
                }
                tokens.push(Spanned {
                    token: Token::Word(word),
                    line: start_line,
                    column: start_column,
                });
            }
        }
    }

    Ok(tokens)
}

/// Analyseur récursif descendant sur une liste de jetons.
struct Parser<'a> {
    tokens: &'a [Spanned],
    pos: usize,
    eof: (usize, usize),
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Spanned> {
        self.tokens.get(self.pos)
    }

    fn peek_word(&self) -> Option<&str> {
        match self.peek() {
            Some(Spanned {
                token: Token::Word(word),
                ..
            }) => Some(word),
            _ => None,
        }
    }

    fn advance(&mut self) -> Option<&Spanned> {
        let token = self.tokens.get(self.pos);
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    /// Position courante (ou de fin de fichier si plus de jetons).
    fn here(&self) -> (usize, usize) {
        self.peek()
            .map(|token| (token.line, token.column))
            .unwrap_or(self.eof)
    }

    fn parse_dql(&mut self) -> Result<ParsedDql, QueryError> {
        let mut parsed = ParsedDql {
            source: Source::All,
            condition: None,
            sort: None,
            limit: None,
        };

        if self.peek_word() == Some("FROM") {
            self.advance();
            let (line, column) = self.here();
            let source = self
                .advance()
                .ok_or_else(|| QueryError::at(line, column, "source attendue après FROM"))?;
            parsed.source = Self::parse_source(source)?;
        }

        if self.peek_word() == Some("WHERE") {
            self.advance();
            parsed.condition = Some(self.parse_expr()?);
        }

        if self.peek_word() == Some("SORT") {
            self.advance();
            parsed.sort = Some(self.parse_sort()?);
        }

        if self.peek_word() == Some("LIMIT") {
            self.advance();
            parsed.limit = Some(self.parse_limit()?);
        }

        if let Some(token) = self.peek() {
            return Err(QueryError::at(
                token.line,
                token.column,
                format!("jeton inattendu « {} »", Self::describe_token(&token.token)),
            ));
        }

        Ok(parsed)
    }

    fn parse_source(spanned: &Spanned) -> Result<Source, QueryError> {
        match &spanned.token {
            Token::Word(word) if word == "*" => Ok(Source::All),
            Token::Word(word) if word.starts_with('#') => Ok(Source::Tag(word[1..].to_string())),
            Token::Quoted(folder) => Ok(Source::Folder(folder.clone())),
            _ => Err(QueryError::at(
                spanned.line,
                spanned.column,
                "source invalide : attendu #tag, \"dossier\" ou *",
            )),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, QueryError> {
        let mut left = self.parse_and()?;
        while self.peek_word() == Some("OR") {
            self.advance();
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, QueryError> {
        let mut left = self.parse_not()?;
        while self.peek_word() == Some("AND") {
            self.advance();
            let right = self.parse_not()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr, QueryError> {
        if self.peek_word() == Some("NOT") {
            self.advance();
            return Ok(Expr::Not(Box::new(self.parse_not()?)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, QueryError> {
        if let Some(token) = self.peek() {
            if token.token == Token::LParen {
                self.advance();
                let expr = self.parse_expr()?;
                let (line, column) = self.here();
                let close = self
                    .advance()
                    .ok_or_else(|| QueryError::at(line, column, "parenthèse fermante attendue"))?;
                if close.token != Token::RParen {
                    return Err(QueryError::at(
                        close.line,
                        close.column,
                        "parenthèse fermante attendue",
                    ));
                }
                return Ok(expr);
            }
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, QueryError> {
        let (fline, fcolumn) = self.here();
        let field_spanned = self
            .advance()
            .ok_or_else(|| QueryError::at(fline, fcolumn, "champ attendu"))?;
        let field = Self::parse_field(field_spanned)?;

        let (oline, ocolumn) = self.here();
        let op_spanned = self
            .advance()
            .ok_or_else(|| QueryError::at(oline, ocolumn, "opérateur manquant après le champ"))?;
        let op = Self::parse_op(op_spanned)?;

        let (vline, vcolumn) = self.here();
        let value_spanned = self
            .advance()
            .ok_or_else(|| QueryError::at(vline, vcolumn, "valeur attendue après l'opérateur"))?;
        let value = Self::parse_literal(value_spanned)?;

        Ok(Expr::Cmp { field, op, value })
    }

    fn parse_field(spanned: &Spanned) -> Result<FieldRef, QueryError> {
        let Token::Word(word) = &spanned.token else {
            return Err(QueryError::at(
                spanned.line,
                spanned.column,
                "champ attendu",
            ));
        };
        if let Some(rest) = word.strip_prefix("file.") {
            let field = match rest {
                "name" => FileField::Name,
                "path" => FileField::Path,
                "folder" => FileField::Folder,
                "size" => FileField::Size,
                "mtime" => FileField::Mtime,
                "ctime" => FileField::Ctime,
                "ext" => FileField::Ext,
                "tags" => FileField::Tags,
                "links" => FileField::Links,
                _ => {
                    return Err(QueryError::at(
                        spanned.line,
                        spanned.column,
                        format!("champ inconnu « {word} »"),
                    ));
                }
            };
            Ok(FieldRef::File(field))
        } else {
            Ok(FieldRef::Property(word.clone()))
        }
    }

    fn parse_op(spanned: &Spanned) -> Result<CmpOp, QueryError> {
        match &spanned.token {
            Token::Eq => Ok(CmpOp::Eq),
            Token::Neq => Ok(CmpOp::Neq),
            Token::Lt => Ok(CmpOp::Lt),
            Token::Gt => Ok(CmpOp::Gt),
            Token::Leq => Ok(CmpOp::Leq),
            Token::Geq => Ok(CmpOp::Geq),
            Token::Word(word) => match word.as_str() {
                "contains" => Ok(CmpOp::Contains),
                "startswith" => Ok(CmpOp::Startswith),
                "endswith" => Ok(CmpOp::Endswith),
                _ => Err(QueryError::at(
                    spanned.line,
                    spanned.column,
                    format!("opérateur manquant : « {word} » n'est pas un opérateur"),
                )),
            },
            _ => Err(QueryError::at(
                spanned.line,
                spanned.column,
                "opérateur manquant",
            )),
        }
    }

    fn parse_literal(spanned: &Spanned) -> Result<Literal, QueryError> {
        match &spanned.token {
            Token::Quoted(text) => Ok(Literal::Text(text.clone())),
            Token::Word(word) => {
                if let Ok(number) = word.parse::<f64>() {
                    Ok(Literal::Number(number))
                } else {
                    Ok(Literal::Text(word.clone()))
                }
            }
            _ => Err(QueryError::at(
                spanned.line,
                spanned.column,
                "valeur attendue",
            )),
        }
    }

    fn parse_sort(&mut self) -> Result<SortSpec, QueryError> {
        let (fline, fcolumn) = self.here();
        let field_spanned = self
            .advance()
            .ok_or_else(|| QueryError::at(fline, fcolumn, "champ attendu après SORT"))?;
        let field = Self::parse_field(field_spanned)?;

        let mut ascending = true;
        if let Some(direction) = self.peek_word() {
            match direction {
                "ASC" => {
                    self.advance();
                }
                "DESC" => {
                    self.advance();
                    ascending = false;
                }
                _ => {}
            }
        }

        Ok(SortSpec { field, ascending })
    }

    fn parse_limit(&mut self) -> Result<usize, QueryError> {
        let (line, column) = self.here();
        let spanned = self
            .advance()
            .ok_or_else(|| QueryError::at(line, column, "nombre attendu après LIMIT"))?;
        let Token::Word(word) = &spanned.token else {
            return Err(QueryError::at(
                spanned.line,
                spanned.column,
                "nombre invalide pour LIMIT",
            ));
        };
        word.parse::<usize>()
            .map_err(|_| QueryError::at(spanned.line, spanned.column, "nombre invalide pour LIMIT"))
    }

    fn describe_token(token: &Token) -> String {
        match token {
            Token::Word(word) => word.clone(),
            Token::Quoted(_) => "\"...\"".to_string(),
            Token::Eq => "=".to_string(),
            Token::Neq => "!=".to_string(),
            Token::Lt => "<".to_string(),
            Token::Gt => ">".to_string(),
            Token::Leq => "<=".to_string(),
            Token::Geq => ">=".to_string(),
            Token::LParen => "(".to_string(),
            Token::RParen => ")".to_string(),
        }
    }
}

/// Analyse une requête Dataview (sous-ensemble de DQL).
///
/// Retourne une erreur localisée (ligne/colonne) pour toute requête invalide :
/// champ `file.*` inconnu, opérateur manquant, guillemet non fermé, nombre
/// invalide pour `LIMIT`, ou clause inconnue.
pub fn parse_dql(query: &str) -> Result<ParsedDql, QueryError> {
    let tokens = lex(query)?;
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        eof: eof_position(query),
    };
    parser.parse_dql()
}

/// Valeur d'un champ résolu pour une note donnée.
#[derive(Debug, Clone, PartialEq)]
enum DqlValue {
    Text(String),
    Number(f64),
    Bool(bool),
    List(Vec<DqlValue>),
}

/// Métadonnées système de fichiers (si le chemin existe sur disque).
fn file_stat(note: &Note) -> Option<std::fs::Metadata> {
    fs::metadata(&note.path).ok()
}

/// Dossier (chemin relatif, séparateurs `/`) contenant `note`.
fn folder_of(note: &Note) -> String {
    note.path
        .parent()
        .map(|parent| {
            parent
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/")
        })
        .unwrap_or_default()
}

/// Convertit une valeur YAML en [`DqlValue`]. Les mappings et `null` sont
/// ignorés (champ considéré absent).
fn from_yaml(value: &Value) -> Option<DqlValue> {
    match value {
        Value::String(text) => Some(DqlValue::Text(text.clone())),
        Value::Number(number) => number.as_f64().map(DqlValue::Number),
        Value::Bool(boolean) => Some(DqlValue::Bool(*boolean)),
        Value::Sequence(sequence) => Some(DqlValue::List(
            sequence.iter().filter_map(from_yaml).collect(),
        )),
        _ => None,
    }
}

/// Résout la valeur de `field` pour `note`, ou `None` si absente.
fn resolve_field(note: &Note, field: &FieldRef) -> Option<DqlValue> {
    match field {
        FieldRef::File(file_field) => match file_field {
            FileField::Name => Some(DqlValue::Text(note.name.clone())),
            FileField::Path => Some(DqlValue::Text(note.path.display().to_string())),
            FileField::Folder => Some(DqlValue::Text(folder_of(note))),
            FileField::Ext => Some(DqlValue::Text(
                note.path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .unwrap_or("")
                    .to_string(),
            )),
            FileField::Size => {
                file_stat(note).map(|metadata| DqlValue::Number(metadata.len() as f64))
            }
            FileField::Mtime => file_stat(note)
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| DqlValue::Number(duration.as_secs_f64())),
            FileField::Ctime => file_stat(note)
                .and_then(|metadata| metadata.created().ok())
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| DqlValue::Number(duration.as_secs_f64())),
            FileField::Tags => Some(DqlValue::List(
                note.metadata
                    .tags
                    .iter()
                    .cloned()
                    .map(DqlValue::Text)
                    .collect(),
            )),
            FileField::Links => Some(DqlValue::List(
                note.metadata
                    .wikilinks
                    .iter()
                    .map(|link| DqlValue::Text(link.target.clone()))
                    .collect(),
            )),
        },
        FieldRef::Property(name) => field_value(note, name).and_then(from_yaml),
    }
}

/// Forme texte d'une valeur, utilisée par les comparaisons.
fn text_of(value: &DqlValue) -> String {
    match value {
        DqlValue::Text(text) => text.clone(),
        DqlValue::Number(number) => number.to_string(),
        DqlValue::Bool(boolean) => boolean.to_string(),
        DqlValue::List(items) => items.iter().map(text_of).collect::<Vec<_>>().join(", "),
    }
}

/// Forme numérique d'une valeur, si lisible.
fn number_of(value: &DqlValue) -> Option<f64> {
    match value {
        DqlValue::Number(number) => Some(*number),
        DqlValue::Text(text) => text.parse().ok(),
        _ => None,
    }
}

fn literal_text(literal: &Literal) -> String {
    match literal {
        Literal::Text(text) => text.clone(),
        Literal::Number(number) => number.to_string(),
    }
}

fn literal_number(literal: &Literal) -> Option<f64> {
    match literal {
        Literal::Number(number) => Some(*number),
        Literal::Text(text) => text.parse().ok(),
    }
}

/// Éléments à comparer : la liste elle-même pour un champ liste, sinon un
/// unique élément.
fn field_items(value: &DqlValue) -> Vec<&DqlValue> {
    match value {
        DqlValue::List(items) => items.iter().collect(),
        other => vec![other],
    }
}

/// Applique `op` entre la valeur du champ et le littéral `rhs`.
fn apply_cmp(op: &CmpOp, field: &DqlValue, rhs: &Literal) -> bool {
    match op {
        CmpOp::Eq => field_items(field)
            .iter()
            .any(|item| text_of(item) == literal_text(rhs)),
        CmpOp::Neq => !field_items(field)
            .iter()
            .any(|item| text_of(item) == literal_text(rhs)),
        CmpOp::Contains => {
            let needle = literal_text(rhs);
            field_items(field)
                .iter()
                .any(|item| text_of(item).contains(&needle))
        }
        CmpOp::Startswith => {
            let prefix = literal_text(rhs);
            field_items(field)
                .iter()
                .any(|item| text_of(item).starts_with(&prefix))
        }
        CmpOp::Endswith => {
            let suffix = literal_text(rhs);
            field_items(field)
                .iter()
                .any(|item| text_of(item).ends_with(&suffix))
        }
        CmpOp::Lt | CmpOp::Gt | CmpOp::Leq | CmpOp::Geq => {
            let items = field_items(field);
            let Some(item) = items.first() else {
                return false;
            };
            let order = match (number_of(item), literal_number(rhs)) {
                (Some(left), Some(right)) => left.partial_cmp(&right).unwrap_or(Ordering::Equal),
                _ => text_of(item).cmp(&literal_text(rhs)),
            };
            match op {
                CmpOp::Lt => order == Ordering::Less,
                CmpOp::Gt => order == Ordering::Greater,
                CmpOp::Leq => order != Ordering::Greater,
                CmpOp::Geq => order != Ordering::Less,
                _ => false,
            }
        }
    }
}

/// Évalue une expression sur une note.
fn eval_expr(expr: &Expr, note: &Note) -> bool {
    match expr {
        Expr::And(left, right) => eval_expr(left, note) && eval_expr(right, note),
        Expr::Or(left, right) => eval_expr(left, note) || eval_expr(right, note),
        Expr::Not(inner) => !eval_expr(inner, note),
        Expr::Cmp { field, op, value } => {
            let Some(field_value) = resolve_field(note, field) else {
                return false;
            };
            apply_cmp(op, &field_value, value)
        }
    }
}

/// Vérifie qu'une note appartient à la source `FROM`.
fn source_matches(source: &Source, note: &Note) -> bool {
    match source {
        Source::All => true,
        Source::Tag(tag) => note
            .metadata
            .tags
            .iter()
            .any(|stored| stored == tag || stored.starts_with(&format!("{tag}/"))),
        Source::Folder(folder) => {
            let note_folder = folder_of(note);
            note_folder == *folder || note_folder.starts_with(&format!("{folder}/"))
        }
    }
}

/// Compare deux valeurs pour le tri : numérique si les deux sont lisibles, texte
/// sinon.
fn compare_values_for_sort(left: &DqlValue, right: &DqlValue) -> Ordering {
    match (number_of(left), number_of(right)) {
        (Some(left), Some(right)) => left.partial_cmp(&right).unwrap_or(Ordering::Equal),
        _ => text_of(left).cmp(&text_of(right)),
    }
}

/// Trie `indices` selon `sort` (stable ; les notes sans valeur passent en fin).
fn sort_indices(notes: &[Note], sort: &SortSpec, indices: &mut [usize]) {
    indices.sort_by(|&left, &right| {
        let left_value = resolve_field(&notes[left], &sort.field);
        let right_value = resolve_field(&notes[right], &sort.field);

        let presence = match (left_value.is_some(), right_value.is_some()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => Ordering::Equal,
        };
        if presence != Ordering::Equal {
            return presence;
        }

        let order = match (left_value, right_value) {
            (Some(left), Some(right)) => compare_values_for_sort(&left, &right),
            _ => Ordering::Equal,
        };
        if order == Ordering::Equal {
            left.cmp(&right)
        } else if sort.ascending {
            order
        } else {
            order.reverse()
        }
    });
}

/// Évalue une requête parsée sur un ensemble de notes donné en entrée.
///
/// Retourne les indices (dans `notes`) des notes qui satisfont la requête,
/// dans l'ordre d'origine, puis triées (le cas échéant) et limitées.
pub fn evaluate_dql(notes: &[Note], parsed: &ParsedDql) -> Vec<usize> {
    let mut indices: Vec<usize> = notes
        .iter()
        .enumerate()
        .filter(|(_, note)| source_matches(&parsed.source, note))
        .map(|(index, _)| index)
        .collect();

    if let Some(condition) = &parsed.condition {
        indices.retain(|&index| eval_expr(condition, &notes[index]));
    }

    if let Some(sort) = &parsed.sort {
        sort_indices(notes, sort, &mut indices);
    }

    if let Some(limit) = parsed.limit {
        indices.truncate(limit);
    }

    indices
}

/// Parse puis évalue une requête Dataview sur `notes`.
///
/// Une requête vide renvoie toutes les notes, dans l'ordre d'origine.
pub fn query_notes(notes: &[Note], query: &str) -> Result<Vec<usize>, QueryError> {
    let parsed = parse_dql(query)?;
    Ok(evaluate_dql(notes, &parsed))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::vault::frontmatter::parse_frontmatter;
    use crate::vault::note::NoteMetadata;

    /// Construit une note à partir de son frontmatter YAML.
    fn note(path: &str, frontmatter: &str) -> Note {
        let source = format!("---\n{frontmatter}---\n\nCorps du document.");
        let frontmatter = parse_frontmatter(&source)
            .expect("frontmatter valide")
            .expect("frontmatter présent");

        Note {
            path: PathBuf::from(path),
            name: Path::new(path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap()
                .to_string(),
            metadata: NoteMetadata {
                frontmatter: Some(frontmatter),
                frontmatter_error: None,
                wikilinks: Vec::new(),
                tags: Vec::new(),
            },
        }
    }

    /// Notes de test :
    ///
    /// | indice | path      | priority | title       |
    /// |--------|-----------|----------|-------------|
    /// | 0      | a.md      | 5        | Alpha       |
    /// | 1      | b.md      | 3        | Beta        |
    /// | 2      | c.md      | 5        | Alpha Beta  |
    /// | 3      | d.md      | —        | Delta       |
    /// | 4      | e.md      | 10       | Epsilon     |
    fn notes() -> Vec<Note> {
        vec![
            note("a.md", "priority: 5\ntitle: Alpha\n"),
            note("b.md", "priority: 3\ntitle: Beta\n"),
            note("c.md", "priority: 5\ntitle: Alpha Beta\n"),
            note("d.md", "title: Delta\n"),
            note("e.md", "priority: 10\ntitle: Epsilon\n"),
        ]
    }

    #[test]
    fn eq_reussit_sur_valeur_texte() {
        let notes = notes();
        assert_eq!(
            filter_notes(&notes, &[Query::new("title", FieldCmp::Eq("Alpha".into()))]),
            vec![0]
        );
    }

    #[test]
    fn eq_echec_sur_valeur_differente() {
        let notes = notes();
        assert!(filter_notes(
            &notes,
            &[Query::new("title", FieldCmp::Eq("Inexistant".into()))]
        )
        .is_empty());
    }

    #[test]
    fn eq_insensible_a_la_valeur_d_un_autre_champ() {
        let notes = notes();
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Eq("5".into()))]),
            vec![0, 2]
        );
    }

    #[test]
    fn contient_reussit_en_sous_chaine() {
        let notes = notes();
        assert_eq!(
            filter_notes(
                &notes,
                &[Query::new("title", FieldCmp::Contains("Alpha".into()))]
            ),
            vec![0, 2]
        );
    }

    #[test]
    fn contient_echec_si_absent() {
        let notes = notes();
        assert!(filter_notes(
            &notes,
            &[Query::new("title", FieldCmp::Contains("Z".into()))]
        )
        .is_empty());
    }

    #[test]
    fn superieur_strict_numerique() {
        let notes = notes();
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Gt(4.0))]),
            vec![0, 2, 4]
        );
    }

    #[test]
    fn superieur_strict_exclut_la_borne() {
        let notes = notes();
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Gt(5.0))]),
            vec![4]
        );
    }

    #[test]
    fn inferieur_strict_numerique() {
        let notes = notes();
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Lt(4.0))]),
            vec![1]
        );
    }

    #[test]
    fn champ_absent_exclu_du_filtrage() {
        let notes = notes();
        // `d` n'a pas de `priority` : elle doit disparaître du résultat.
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Gt(0.0))]),
            vec![0, 1, 2, 4]
        );
    }

    #[test]
    fn frontmatter_absent_exclu_du_filtrage() {
        let notes = notes();
        let sans = note("f.md", "");
        let mut notes = notes;
        notes.push(sans);
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Gt(0.0))]),
            vec![0, 1, 2, 4]
        );
    }

    #[test]
    fn conjonction_et_logique() {
        let notes = notes();
        // `Alpha` dans le titre ET priority > 4 → a (5) et c (5).
        assert_eq!(
            filter_notes(
                &notes,
                &[
                    Query::new("title", FieldCmp::Contains("Alpha".into())),
                    Query::new("priority", FieldCmp::Gt(4.0)),
                ]
            ),
            vec![0, 2]
        );
    }

    #[test]
    fn conjonction_exclut_dès_qu_une_requete_echoue() {
        let notes = notes();
        // priority > 4 exclut b (3) ; title == Beta exclut le reste.
        assert!(filter_notes(
            &notes,
            &[
                Query::new("priority", FieldCmp::Gt(4.0)),
                Query::new("title", FieldCmp::Eq("Beta".into())),
            ]
        )
        .is_empty());
    }

    #[test]
    fn requete_vide_retourne_toutes_les_notes() {
        let notes = notes();
        assert_eq!(filter_notes(&notes, &[]), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn tri_numerique_croissant_et_sans_champ_en_fin() {
        let notes = notes();
        let mut indices = vec![2, 0, 4, 1, 3];
        sort_by_field(&notes, "priority", true, &mut indices);
        // 3 (b), puis 5 (a, c, ordre d'origine), puis 10 (e), enfin d (absent).
        assert_eq!(indices, vec![1, 0, 2, 4, 3]);
    }

    #[test]
    fn tri_numerique_decroissant_et_sans_champ_en_fin() {
        let notes = notes();
        let mut indices = vec![0, 1, 2, 3, 4];
        sort_by_field(&notes, "priority", false, &mut indices);
        // 10, 5, 5, 3, puis d (absent) toujours en fin.
        assert_eq!(indices, vec![4, 0, 2, 1, 3]);
    }

    #[test]
    fn tri_texte_croissant() {
        let notes = notes();
        let mut indices = vec![4, 3, 2, 1, 0];
        sort_by_field(&notes, "title", true, &mut indices);
        assert_eq!(indices, vec![0, 2, 1, 3, 4]);
    }

    #[test]
    fn tri_texte_decroissant() {
        let notes = notes();
        let mut indices = vec![0, 1, 2, 3, 4];
        sort_by_field(&notes, "title", false, &mut indices);
        assert_eq!(indices, vec![4, 3, 1, 2, 0]);
    }

    #[test]
    fn tri_stable_a_egalite() {
        let notes = notes();
        // `a` et `c` partagent priority = 5 ; placés dans l'ordre inverse,
        // l'ordre d'origine doit être restauré par la stabilité.
        let mut indices = vec![2, 0];
        sort_by_field(&notes, "priority", true, &mut indices);
        assert_eq!(indices, vec![0, 2]);
    }

    #[test]
    fn tri_stable_a_egalite_sens_decroissant() {
        let notes = notes();
        let mut indices = vec![2, 0];
        sort_by_field(&notes, "priority", false, &mut indices);
        assert_eq!(indices, vec![0, 2]);
    }

    #[test]
    fn tri_vide_ne_fait_rien() {
        let notes = notes();
        let mut indices = Vec::new();
        sort_by_field(&notes, "priority", true, &mut indices);
        assert!(indices.is_empty());
    }

    #[test]
    fn type_mixte_nombre_et_chaine_numerique() {
        let notes = vec![
            note("a.md", "priority: 5\n"),
            note("b.md", "priority: \"5\"\n"),
            note("c.md", "priority: \"10\"\n"),
            note("d.md", "priority: \"abc\"\n"),
        ];
        let mut indices = vec![3, 2, 1, 0];
        sort_by_field(&notes, "priority", true, &mut indices);
        // 5 (nombre), 5 (chaîne), 10 (chaîne), puis « abc » en texte.
        assert_eq!(indices, vec![0, 1, 2, 3]);
    }

    #[test]
    fn filtre_sur_chaine_numerique() {
        let notes = vec![
            note("a.md", "priority: \"5\"\n"),
            note("b.md", "priority: \"12\"\n"),
            note("c.md", "priority: abc\n"),
        ];
        // Gt(4) : 5 et 12 sont numériques, abc ne l'est pas → exclue.
        assert_eq!(
            filter_notes(&notes, &[Query::new("priority", FieldCmp::Gt(4.0))]),
            vec![0, 1]
        );
    }

    #[test]
    fn valeur_null_est_presente_mais_vide() {
        let notes = vec![note("a.md", "title: \n"), note("b.md", "title: ok\n")];
        // `title: ` donne un `Value::Null` : présent, donc non exclu par Eq(""),
        // mais différent de « ok ».
        assert_eq!(
            filter_notes(&notes, &[Query::new("title", FieldCmp::Eq("".into()))]),
            vec![0]
        );
        assert_eq!(
            filter_notes(
                &notes,
                &[Query::new("title", FieldCmp::Contains("".into()))]
            ),
            vec![0, 1]
        );
    }

    #[test]
    fn sequence_serialisee_en_yaml_compact() {
        let notes = vec![
            note("a.md", "tags:\n  - rust\n  - test\n"),
            note("b.md", "tags: [rust, docs]\n"),
        ];
        assert_eq!(
            filter_notes(
                &notes,
                &[Query::new("tags", FieldCmp::Contains("rust".into()))]
            ),
            vec![0, 1]
        );
        assert_eq!(
            filter_notes(
                &notes,
                &[Query::new("tags", FieldCmp::Contains("docs".into()))]
            ),
            vec![1]
        );
    }
}
