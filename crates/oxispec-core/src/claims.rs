//! Claims (design section 20, layer 1): the machine-checkable part of a spec.
//!
//! A requirement can carry a fenced `claims` block. Each line says that in any
//! context where the scope holds, the constraint must hold:
//!
//! ```text
//! for role = admin: session.idle_timeout <= 15m
//! ```
//!
//! Variables are declared in a fenced `vocabulary` block, so two requirements
//! talk about the same thing. A constraint uses one variable, so the solver
//! is exact and needs nothing but sets of values: intervals for numbers and
//! durations, finite sets for enums and booleans.
//!
//! Two claims conflict when some context makes both scopes hold and no value
//! satisfies both constraints. Contexts are found by splitting each scope
//! variable into the regions that its scopes cannot tell apart, so the answer
//! does not depend on which values happen to be written down.
//!
//! E017 a claim cannot be read    E018 the vocabulary cannot be read
//! E019 claims conflict           W005 a SHOULD claim cannot hold
//! W006 a claim never applies     I002 a claim is implied by another

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::PathBuf;

use serde::Deserialize;

use crate::finding::Finding;
use crate::graph::Graph;
use crate::id;

/// More cells than this and the check falls back to pairs of claims.
const MAX_CELLS: usize = 4096;
/// More claims on one variable than this and the redundancy lint is skipped.
const MAX_FOR_REDUNDANCY: usize = 100;

/// Statuses of nodes whose claims do not count. Draft and proposed claims do,
/// because the time to learn about a conflict is when the change is reviewed.
const INACTIVE_STATUSES: &[&str] = &["rejected", "superseded", "deprecated", "withdrawn"];

const RESERVED: &[&str] = &["for", "and", "or", "not", "in", "must", "should"];

// ---------------------------------------------------------------- values

/// A set of values of one variable.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Set {
    /// Sorted, disjoint, inclusive ranges of integers (durations in milliseconds).
    Ranges(Vec<(i64, i64)>),
    /// Positions in the variable's list of enum values.
    Items(BTreeSet<usize>),
}

/// One value, standing for the region it lies in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rep {
    Int(i64),
    Item(usize),
}

fn normalize(mut ranges: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    ranges.retain(|&(lo, hi)| lo <= hi);
    ranges.sort_unstable();
    let mut out: Vec<(i64, i64)> = Vec::with_capacity(ranges.len());
    for (lo, hi) in ranges {
        match out.last_mut() {
            Some(last) if lo <= last.1.saturating_add(1) => last.1 = last.1.max(hi),
            _ => out.push((lo, hi)),
        }
    }
    out
}

impl Set {
    fn is_empty(&self) -> bool {
        match self {
            Set::Ranges(r) => r.is_empty(),
            Set::Items(s) => s.is_empty(),
        }
    }

    fn contains(&self, rep: Rep) -> bool {
        match (self, rep) {
            (Set::Ranges(r), Rep::Int(x)) => r.iter().any(|&(lo, hi)| lo <= x && x <= hi),
            (Set::Items(s), Rep::Item(i)) => s.contains(&i),
            _ => false,
        }
    }

    fn intersect(&self, other: &Set) -> Set {
        match (self, other) {
            (Set::Ranges(a), Set::Ranges(b)) => {
                let (mut i, mut j) = (0, 0);
                let mut out = Vec::new();
                while i < a.len() && j < b.len() {
                    let lo = a[i].0.max(b[j].0);
                    let hi = a[i].1.min(b[j].1);
                    if lo <= hi {
                        out.push((lo, hi));
                    }
                    if a[i].1 < b[j].1 {
                        i += 1;
                    } else {
                        j += 1;
                    }
                }
                Set::Ranges(out)
            }
            (Set::Items(a), Set::Items(b)) => Set::Items(a.intersection(b).copied().collect()),
            _ => unreachable!("a variable has one kind of values"),
        }
    }

    fn union(&self, other: &Set) -> Set {
        match (self, other) {
            (Set::Ranges(a), Set::Ranges(b)) => {
                Set::Ranges(normalize(a.iter().chain(b).copied().collect()))
            }
            (Set::Items(a), Set::Items(b)) => Set::Items(a.union(b).copied().collect()),
            _ => unreachable!("a variable has one kind of values"),
        }
    }

    /// What is in `domain` and not in `self`.
    fn complement_in(&self, domain: &Set) -> Set {
        match (self, domain) {
            (Set::Ranges(s), Set::Ranges(_)) => {
                let mut gaps = Vec::new();
                let mut next = Some(i64::MIN);
                for &(lo, hi) in s {
                    if let Some(from) = next
                        && from < lo
                    {
                        gaps.push((from, lo - 1));
                    }
                    next = hi.checked_add(1);
                }
                if let Some(from) = next {
                    gaps.push((from, i64::MAX));
                }
                Set::Ranges(gaps).intersect(domain)
            }
            (Set::Items(s), Set::Items(d)) => Set::Items(d.difference(s).copied().collect()),
            _ => unreachable!("a variable has one kind of values"),
        }
    }

    fn is_subset_of(&self, other: &Set) -> bool {
        self.intersect(other) == *self
    }
}

// ------------------------------------------------------------- vocabulary

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Int,
    Duration,
    Bool,
    Enum,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Int => "int",
            Kind::Duration => "duration",
            Kind::Bool => "bool",
            Kind::Enum => "enum",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Variable {
    pub name: String,
    pub kind: Kind,
    /// The values of an enum, or `false` and `true`.
    pub values: Vec<String>,
    /// The node that declares it.
    pub node: String,
}

impl Variable {
    fn domain(&self) -> Set {
        match self.kind {
            Kind::Int => Set::Ranges(vec![(i64::MIN, i64::MAX)]),
            Kind::Duration => Set::Ranges(vec![(0, i64::MAX)]),
            Kind::Bool | Kind::Enum => Set::Items((0..self.values.len()).collect()),
        }
    }

    fn is_numeric(&self) -> bool {
        matches!(self.kind, Kind::Int | Kind::Duration)
    }
}

#[derive(Debug, Default)]
pub struct Vocabulary {
    pub variables: BTreeMap<String, Variable>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    #[serde(rename = "type")]
    kind: Kind,
    values: Option<Vec<String>>,
}

fn is_word(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

fn is_reserved(text: &str) -> bool {
    RESERVED.contains(&text.to_ascii_lowercase().as_str())
}

fn variable_from(name: &str, spec: Spec, node: &str) -> Result<Variable, String> {
    if !is_word(name) || name.contains('-') || is_reserved(name) {
        return Err(format!(
            "`{name}` is not a variable name: use letters, digits, `_` and `.`, and not a keyword"
        ));
    }
    let values = match (spec.kind, spec.values) {
        (Kind::Bool, None) => vec!["false".to_string(), "true".to_string()],
        (Kind::Bool, Some(_)) => return Err(format!("`{name}` is a bool and takes no `values`")),
        (Kind::Enum, Some(values)) => {
            if values.is_empty() {
                return Err(format!("`{name}` lists no values"));
            }
            if let Some(bad) = values.iter().find(|v| !is_word(v) || is_reserved(v)) {
                return Err(format!(
                    "`{bad}` is not a value name for `{name}`: use letters, digits, `_`, `.` and `-`, and not a keyword"
                ));
            }
            let unique: HashSet<_> = values.iter().collect();
            if unique.len() != values.len() {
                return Err(format!("`{name}` lists a value twice"));
            }
            values
        }
        (Kind::Enum, None) => return Err(format!("`{name}` is an enum and needs `values`")),
        (_, Some(_)) => return Err(format!("`{name}` is a number and takes no `values`")),
        (_, None) => Vec::new(),
    };
    Ok(Variable {
        name: name.to_string(),
        kind: spec.kind,
        values,
        node: node.to_string(),
    })
}

fn read_vocabulary(graph: &Graph, out: &mut Vec<Finding>) -> Vocabulary {
    let mut vocab = Vocabulary::default();
    for node in &graph.nodes {
        for block in &node.vocabulary {
            let yaml = block.text.trim();
            if yaml.lines().all(|l| {
                let l = l.trim();
                l.is_empty() || l.starts_with('#')
            }) {
                continue;
            }
            let bad = |message: String| {
                Finding::error("E018", message)
                    .at(&node.path, Some(block.line))
                    .node(&node.id)
            };
            let specs: BTreeMap<String, Spec> = match serde_saphyr::from_str(yaml) {
                Ok(specs) => specs,
                Err(e) => {
                    out.push(bad(format!("the vocabulary is not valid: {e}")));
                    continue;
                }
            };
            for (name, spec) in specs {
                match variable_from(&name, spec, &node.id) {
                    Err(message) => out.push(bad(message)),
                    Ok(var) => {
                        if let Some(first) = vocab.variables.get(&name) {
                            out.push(bad(format!(
                                "variable `{name}` is declared twice (first in {})",
                                first.node
                            )));
                        } else {
                            vocab.variables.insert(name, var);
                        }
                    }
                }
            }
        }
    }
    vocab
}

// ------------------------------------------------------------------ lexer

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    Num { value: i64, unit: bool },
    Cmp(Cmp),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Colon,
    Star,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

type Spanned = (Tok, usize, usize);

fn unit_ms(unit: &str) -> Option<i64> {
    Some(match unit {
        "ms" => 1,
        "s" => 1_000,
        "m" => 60_000,
        "h" => 3_600_000,
        "d" => 86_400_000,
        "w" => 604_800_000,
        _ => return None,
    })
}

fn lex(text: &str) -> Result<Vec<Spanned>, String> {
    let b = text.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        let start = i;
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let tok = if c.is_ascii_digit()
            || (c == b'-' && b.get(i + 1).is_some_and(u8::is_ascii_digit))
        {
            i += 1;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let digits = &text[start..i];
            let unit_start = i;
            while i < b.len() && b[i].is_ascii_alphabetic() {
                i += 1;
            }
            let unit = &text[unit_start..i];
            let mut value: i64 = digits
                .parse()
                .map_err(|_| format!("`{digits}` is too large"))?;
            if !unit.is_empty() {
                let factor = unit_ms(unit)
                    .ok_or_else(|| format!("unknown unit `{unit}` (use ms, s, m, h, d or w)"))?;
                value = value
                    .checked_mul(factor)
                    .ok_or_else(|| format!("`{digits}{unit}` is too large"))?;
            }
            Tok::Num {
                value,
                unit: !unit.is_empty(),
            }
        } else if c.is_ascii_alphabetic() || c == b'_' {
            i += 1;
            while i < b.len()
                && (b[i].is_ascii_alphanumeric() || matches!(b[i], b'_' | b'.' | b'-'))
            {
                i += 1;
            }
            Tok::Word(text[start..i].to_string())
        } else {
            i += 1;
            let next = b.get(i).copied();
            match (c, next) {
                (b'=', Some(b'=')) => {
                    i += 1;
                    Tok::Cmp(Cmp::Eq)
                }
                (b'=', _) => Tok::Cmp(Cmp::Eq),
                (b'!', Some(b'=')) => {
                    i += 1;
                    Tok::Cmp(Cmp::Ne)
                }
                (b'<', Some(b'=')) => {
                    i += 1;
                    Tok::Cmp(Cmp::Le)
                }
                (b'<', _) => Tok::Cmp(Cmp::Lt),
                (b'>', Some(b'=')) => {
                    i += 1;
                    Tok::Cmp(Cmp::Ge)
                }
                (b'>', _) => Tok::Cmp(Cmp::Gt),
                (b'(', _) => Tok::LParen,
                (b')', _) => Tok::RParen,
                (b'[', _) => Tok::LBracket,
                (b']', _) => Tok::RBracket,
                (b',', _) => Tok::Comma,
                (b':', _) => Tok::Colon,
                (b'*', _) => Tok::Star,
                _ => {
                    let ch = text[start..].chars().next().unwrap_or('?');
                    return Err(format!("unexpected `{ch}`"));
                }
            }
        };
        out.push((tok, start, i));
    }
    Ok(out)
}

// ----------------------------------------------------------------- parser

#[derive(Debug, Clone)]
enum Lit {
    Num { value: i64, unit: bool },
    Word(String),
}

#[derive(Debug)]
enum Raw {
    /// `var op literal`, `var in [..]`, or a bare boolean `var`.
    Test {
        var: String,
        op: RawOp,
        lits: Vec<Lit>,
    },
    Not(Box<Raw>),
    And(Vec<Raw>),
    Or(Vec<Raw>),
}

#[derive(Debug, Clone, Copy)]
enum RawOp {
    Cmp(Cmp),
    In,
    Is,
}

struct Parser {
    toks: Vec<Spanned>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.0)
    }

    fn keyword(&self, word: &str) -> bool {
        matches!(self.peek(), Some(Tok::Word(w)) if w.eq_ignore_ascii_case(word))
    }

    fn eat_keyword(&mut self, word: &str) -> bool {
        let found = self.keyword(word);
        if found {
            self.pos += 1;
        }
        found
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        let found = self.peek() == Some(tok);
        if found {
            self.pos += 1;
        }
        found
    }

    fn expr(&mut self) -> Result<Raw, String> {
        let mut parts = vec![self.and()?];
        while self.eat_keyword("or") {
            parts.push(self.and()?);
        }
        Ok(if parts.len() == 1 {
            parts.remove(0)
        } else {
            Raw::Or(parts)
        })
    }

    fn and(&mut self) -> Result<Raw, String> {
        let mut parts = vec![self.unary()?];
        while self.eat_keyword("and") {
            parts.push(self.unary()?);
        }
        Ok(if parts.len() == 1 {
            parts.remove(0)
        } else {
            Raw::And(parts)
        })
    }

    fn unary(&mut self) -> Result<Raw, String> {
        if self.eat_keyword("not") {
            return Ok(Raw::Not(Box::new(self.unary()?)));
        }
        if self.eat(&Tok::LParen) {
            let inner = self.expr()?;
            if !self.eat(&Tok::RParen) {
                return Err("a `(` is not closed".to_string());
            }
            return Ok(inner);
        }
        self.test()
    }

    fn literal(&mut self) -> Result<Lit, String> {
        match self.toks.get(self.pos).map(|t| t.0.clone()) {
            Some(Tok::Num { value, unit }) => {
                self.pos += 1;
                Ok(Lit::Num { value, unit })
            }
            Some(Tok::Word(w)) if !is_reserved(&w) => {
                self.pos += 1;
                Ok(Lit::Word(w))
            }
            Some(_) => Err("expected a value".to_string()),
            None => Err("the line ends where a value should be".to_string()),
        }
    }

    fn test(&mut self) -> Result<Raw, String> {
        let var = match self.toks.get(self.pos).map(|t| t.0.clone()) {
            Some(Tok::Word(w)) if !is_reserved(&w) => {
                self.pos += 1;
                w
            }
            Some(Tok::Word(w)) => return Err(format!("`{w}` is a keyword, not a variable")),
            Some(_) => return Err("expected a variable".to_string()),
            None => return Err("the line ends where a variable should be".to_string()),
        };
        if let Some(Tok::Cmp(op)) = self.peek().cloned() {
            self.pos += 1;
            let lit = self.literal()?;
            return Ok(Raw::Test {
                var,
                op: RawOp::Cmp(op),
                lits: vec![lit],
            });
        }
        let negated = self.keyword("not") && {
            self.pos += 1;
            true
        };
        if self.eat_keyword("in") {
            if !self.eat(&Tok::LBracket) {
                return Err("`in` needs a list such as [a, b]".to_string());
            }
            let mut lits = vec![self.literal()?];
            while self.eat(&Tok::Comma) {
                lits.push(self.literal()?);
            }
            if !self.eat(&Tok::RBracket) {
                return Err("a `[` is not closed".to_string());
            }
            let test = Raw::Test {
                var,
                op: RawOp::In,
                lits,
            };
            return Ok(if negated {
                Raw::Not(Box::new(test))
            } else {
                test
            });
        }
        if negated {
            return Err("`not` here needs `in`, as in `x not in [a, b]`".to_string());
        }
        Ok(Raw::Test {
            var,
            op: RawOp::Is,
            lits: Vec::new(),
        })
    }
}

// ------------------------------------------------------------------ typing

/// A test with its variable and values resolved.
#[derive(Debug, Clone)]
enum Typed {
    Atom { var: String, set: Set, domain: Set },
    Not(Box<Typed>),
    And(Vec<Typed>),
    Or(Vec<Typed>),
}

impl Typed {
    /// The values that satisfy it, and the domain they are taken from. All
    /// atoms of a constraint use one variable, so there is one domain.
    fn values(&self) -> (Set, Set) {
        match self {
            Typed::Atom { set, domain, .. } => (set.clone(), domain.clone()),
            Typed::Not(inner) => {
                let (set, domain) = inner.values();
                (set.complement_in(&domain), domain)
            }
            Typed::And(parts) | Typed::Or(parts) => {
                let mut it = parts.iter().map(Typed::values);
                let (mut acc, domain) = it.next().expect("a connective has parts");
                for (set, _) in it {
                    acc = if matches!(self, Typed::And(_)) {
                        acc.intersect(&set)
                    } else {
                        acc.union(&set)
                    };
                }
                (acc, domain)
            }
        }
    }

    fn holds(&self, at: &dyn Fn(&str) -> Rep) -> bool {
        match self {
            Typed::Atom { var, set, .. } => set.contains(at(var)),
            Typed::Not(inner) => !inner.holds(at),
            Typed::And(parts) => parts.iter().all(|p| p.holds(at)),
            Typed::Or(parts) => parts.iter().any(|p| p.holds(at)),
        }
    }

    fn atoms<'a>(&'a self, out: &mut BTreeMap<&'a str, Vec<&'a Set>>) {
        match self {
            Typed::Atom { var, set, .. } => out.entry(var).or_default().push(set),
            Typed::Not(inner) => inner.atoms(out),
            Typed::And(parts) | Typed::Or(parts) => parts.iter().for_each(|p| p.atoms(out)),
        }
    }

    fn variables(&self) -> BTreeSet<&str> {
        let mut atoms = BTreeMap::new();
        self.atoms(&mut atoms);
        atoms.into_keys().collect()
    }
}

fn number(var: &Variable, lit: &Lit) -> Result<i64, String> {
    match (var.kind, lit) {
        (Kind::Int, Lit::Num { value, unit: false }) => Ok(*value),
        (Kind::Int, Lit::Num { unit: true, .. }) => Err(format!(
            "`{}` is a number: write it without a unit",
            var.name
        )),
        (Kind::Duration, Lit::Num { value, unit: true }) => Ok(*value),
        (Kind::Duration, Lit::Num { unit: false, .. }) => Err(format!(
            "`{}` is a duration: write a unit, for example 15m",
            var.name
        )),
        (_, Lit::Word(w)) => Err(format!("`{w}` is not a number, and `{}` is", var.name)),
        _ => unreachable!("only numeric variables get here"),
    }
}

fn atom(vocab: &Vocabulary, name: &str, op: RawOp, lits: &[Lit]) -> Result<Typed, String> {
    let var = vocab.variables.get(name).ok_or_else(|| {
        format!("unknown variable `{name}`: declare it in a `vocabulary` block (`oxispec new vocabulary`)")
    })?;
    let domain = var.domain();
    let set = if var.is_numeric() {
        let (RawOp::Cmp(_) | RawOp::In) = op else {
            return Err(format!(
                "`{name}` needs a comparison, such as `{name} <= 5`"
            ));
        };
        let mut values = Vec::new();
        for lit in lits {
            values.push(number(var, lit)?);
        }
        let v = values[0];
        let ranges = match op {
            RawOp::Cmp(Cmp::Eq) => vec![(v, v)],
            RawOp::Cmp(Cmp::Ne) => {
                return Ok(Typed::Not(Box::new(Typed::Atom {
                    var: name.to_string(),
                    set: Set::Ranges(vec![(v, v)]),
                    domain,
                })));
            }
            RawOp::Cmp(Cmp::Lt) => match v.checked_sub(1) {
                Some(hi) => vec![(i64::MIN, hi)],
                None => Vec::new(),
            },
            RawOp::Cmp(Cmp::Le) => vec![(i64::MIN, v)],
            RawOp::Cmp(Cmp::Gt) => match v.checked_add(1) {
                Some(lo) => vec![(lo, i64::MAX)],
                None => Vec::new(),
            },
            RawOp::Cmp(Cmp::Ge) => vec![(v, i64::MAX)],
            RawOp::In => values.iter().map(|&x| (x, x)).collect(),
            RawOp::Is => unreachable!("checked above"),
        };
        Set::Ranges(normalize(ranges)).intersect(&domain)
    } else {
        match op {
            RawOp::Is if var.kind == Kind::Bool => Set::Items(BTreeSet::from([1])),
            RawOp::Is => {
                return Err(format!(
                    "`{name}` is an enum: compare it, for example `{name} = {}`",
                    var.values[0]
                ));
            }
            RawOp::Cmp(Cmp::Eq | Cmp::Ne) | RawOp::In => {
                let mut items = BTreeSet::new();
                for lit in lits {
                    let Lit::Word(w) = lit else {
                        return Err(format!("`{name}` takes {}, not a number", list(var)));
                    };
                    let Some(position) = var.values.iter().position(|v| v == w) else {
                        return Err(format!("`{w}` is not a value of `{name}` ({})", list(var)));
                    };
                    items.insert(position);
                }
                let set = Set::Items(items);
                if matches!(op, RawOp::Cmp(Cmp::Ne)) {
                    set.complement_in(&domain)
                } else {
                    set
                }
            }
            RawOp::Cmp(_) => {
                return Err(format!("`{name}` is not a number, so it cannot be ordered"));
            }
        }
    };
    Ok(Typed::Atom {
        var: name.to_string(),
        set,
        domain,
    })
}

fn list(var: &Variable) -> String {
    format!("one of {}", var.values.join(", "))
}

fn compile(vocab: &Vocabulary, raw: &Raw) -> Result<Typed, String> {
    match raw {
        Raw::Test { var, op, lits } => atom(vocab, var, *op, lits),
        Raw::Not(inner) => Ok(Typed::Not(Box::new(compile(vocab, inner)?))),
        Raw::And(parts) => Ok(Typed::And(
            parts
                .iter()
                .map(|p| compile(vocab, p))
                .collect::<Result<_, _>>()?,
        )),
        Raw::Or(parts) => Ok(Typed::Or(
            parts
                .iter()
                .map(|p| compile(vocab, p))
                .collect::<Result<_, _>>()?,
        )),
    }
}

// ----------------------------------------------------------------- claims

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modality {
    Must,
    MustNot,
    Should,
    ShouldNot,
}

impl Modality {
    /// SHOULD claims are soft: they can warn, never fail.
    pub fn is_soft(self) -> bool {
        matches!(self, Modality::Should | Modality::ShouldNot)
    }

    pub fn name(self) -> &'static str {
        match self {
            Modality::Must => "must",
            Modality::MustNot => "must not",
            Modality::Should => "should",
            Modality::ShouldNot => "should not",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Claim {
    pub node: String,
    pub path: PathBuf,
    pub line: usize,
    /// The line as written.
    pub text: String,
    pub modality: Modality,
    /// The variable the constraint is about.
    pub subject: String,
    /// The scope as written, or `None` for `for *`.
    pub scope_text: Option<String>,
    /// Whether it counts: its node is not rejected, superseded or withdrawn.
    pub active: bool,
    scope: Option<Typed>,
    /// The values of the subject the claim allows.
    allowed: Set,
}

struct Parsed {
    modality: Modality,
    scope: Option<(Typed, String)>,
    subject: String,
    allowed: Set,
}

fn parse_claim(vocab: &Vocabulary, text: &str) -> Result<Parsed, String> {
    let mut p = Parser {
        toks: lex(text)?,
        pos: 0,
    };
    let modality = if p.eat_keyword("must") {
        if p.eat_keyword("not") {
            Modality::MustNot
        } else {
            Modality::Must
        }
    } else if p.eat_keyword("should") {
        if p.eat_keyword("not") {
            Modality::ShouldNot
        } else {
            Modality::Should
        }
    } else {
        Modality::Must
    };
    let mut scope = None;
    if p.eat_keyword("for") {
        if p.eat(&Tok::Star) {
            if !p.eat(&Tok::Colon) {
                return Err("expected `:` after the scope".to_string());
            }
        } else {
            let from = p.toks.get(p.pos).map_or(0, |t| t.1);
            let raw = p.expr()?;
            let to = p.toks.get(p.pos.wrapping_sub(1)).map_or(from, |t| t.2);
            if !p.eat(&Tok::Colon) {
                return Err("expected `:` after the scope".to_string());
            }
            scope = Some((compile(vocab, &raw)?, text[from..to].to_string()));
        }
    }
    if p.pos >= p.toks.len() {
        return Err("the claim has no constraint".to_string());
    }
    let raw = p.expr()?;
    if let Some((tok, ..)) = p.toks.get(p.pos) {
        return Err(format!(
            "unexpected {} after the constraint",
            match tok {
                Tok::Word(w) => format!("`{w}`"),
                _ => "text".to_string(),
            }
        ));
    }
    let constraint = compile(vocab, &raw)?;
    let used = constraint.variables();
    if used.len() != 1 {
        return Err(format!(
            "a constraint is about one variable, and this uses {}: write one line for each",
            used.iter()
                .map(|v| format!("`{v}`"))
                .collect::<Vec<_>>()
                .join(" and ")
        ));
    }
    let subject = used.into_iter().next().expect("one variable").to_string();
    if let Some((scope, _)) = &scope
        && scope.variables().contains(subject.as_str())
    {
        return Err(format!(
            "the scope cannot use `{subject}`, the variable the claim constrains"
        ));
    }
    let (mut allowed, domain) = constraint.values();
    if matches!(modality, Modality::MustNot | Modality::ShouldNot) {
        allowed = allowed.complement_in(&domain);
    }
    Ok(Parsed {
        modality,
        scope,
        subject,
        allowed,
    })
}

/// What reading the vocabulary and the claims found.
#[derive(Debug, Default)]
pub struct Analysis {
    pub vocabulary: Vocabulary,
    pub claims: Vec<Claim>,
    /// Problems with the vocabulary and with how claims are written (E017, E018).
    pub findings: Vec<Finding>,
}

fn is_inactive(status: Option<&str>) -> bool {
    status.is_some_and(|s| INACTIVE_STATUSES.contains(&s.to_ascii_lowercase().as_str()))
}

pub fn analyze(graph: &Graph) -> Analysis {
    let mut analysis = Analysis::default();
    if !graph
        .nodes
        .iter()
        .any(|n| !n.claims.is_empty() || !n.vocabulary.is_empty())
    {
        return analysis;
    }
    let vocabulary = read_vocabulary(graph, &mut analysis.findings);
    let superseded: HashSet<&str> = graph
        .nodes
        .iter()
        .flat_map(|n| n.links.get("supersedes").into_iter().flatten())
        .map(|t| id::base_of(t))
        .collect();
    for node in &graph.nodes {
        let active = !is_inactive(node.status.as_deref()) && !superseded.contains(node.id.as_str());
        for line in &node.claims {
            match parse_claim(&vocabulary, &line.text) {
                Ok(parsed) => analysis.claims.push(Claim {
                    node: node.id.clone(),
                    path: node.path.clone(),
                    line: line.line,
                    text: line.text.trim().to_string(),
                    modality: parsed.modality,
                    subject: parsed.subject,
                    scope_text: parsed.scope.as_ref().map(|s| s.1.clone()),
                    active,
                    scope: parsed.scope.map(|s| s.0),
                    allowed: parsed.allowed,
                }),
                Err(message) => analysis.findings.push(
                    Finding::error("E017", format!("claim `{}`: {message}", line.text.trim()))
                        .at(&node.path, Some(line.line))
                        .node(&node.id),
                ),
            }
        }
    }
    analysis.vocabulary = vocabulary;
    analysis
}

// ------------------------------------------------------------------ cells

#[derive(Debug, Clone)]
struct Region {
    rep: Rep,
    label: String,
}

/// The contexts to look at: for each scope variable, regions that no scope
/// can tell apart, and every combination of one region per variable.
struct Space {
    vars: Vec<String>,
    regions: Vec<Vec<Region>>,
    cells: Vec<Vec<usize>>,
}

impl Space {
    fn rep(&self, cell: &[usize], var: &str) -> Rep {
        let at = self
            .vars
            .iter()
            .position(|v| v == var)
            .expect("a scope variable");
        self.regions[at][cell[at]].rep
    }

    fn holds(&self, cell: &[usize], scope: &Option<Typed>) -> bool {
        scope
            .as_ref()
            .is_none_or(|s| s.holds(&|var: &str| self.rep(cell, var)))
    }

    /// Where the cell sits, in terms of the variables `claims` use in their scopes.
    fn describe(&self, cell: &[usize], claims: &[&Claim]) -> String {
        let used: BTreeSet<&str> = claims
            .iter()
            .filter_map(|c| c.scope.as_ref())
            .flat_map(Typed::variables)
            .collect();
        let parts: Vec<&str> = self
            .vars
            .iter()
            .enumerate()
            .filter(|(_, v)| used.contains(v.as_str()))
            .map(|(at, _)| self.regions[at][cell[at]].label.as_str())
            .collect();
        if parts.is_empty() {
            "in every context".to_string()
        } else {
            format!("when {}", parts.join(" and "))
        }
    }
}

pub fn format_duration(ms: i64) -> String {
    if ms == 0 {
        return "0s".to_string();
    }
    for (unit, factor) in [
        ("w", 604_800_000),
        ("d", 86_400_000),
        ("h", 3_600_000),
        ("m", 60_000),
        ("s", 1_000),
    ] {
        if ms % factor == 0 {
            return format!("{}{unit}", ms / factor);
        }
    }
    format!("{ms}ms")
}

fn shown(var: &Variable, value: i64) -> String {
    if var.kind == Kind::Duration {
        format_duration(value)
    } else {
        value.to_string()
    }
}

fn regions(var: &Variable, atoms: &[&Set]) -> Vec<Region> {
    match var.domain() {
        Set::Items(items) => {
            let mut groups: Vec<(Vec<bool>, Vec<usize>)> = Vec::new();
            for item in items {
                let signature: Vec<bool> =
                    atoms.iter().map(|s| s.contains(Rep::Item(item))).collect();
                match groups.iter_mut().find(|g| g.0 == signature) {
                    Some(g) => g.1.push(item),
                    None => groups.push((signature, vec![item])),
                }
            }
            groups
                .into_iter()
                .map(|(_, members)| Region {
                    rep: Rep::Item(members[0]),
                    label: match members.as_slice() {
                        [one] => format!("{} = {}", var.name, var.values[*one]),
                        many => format!(
                            "{} in [{}]",
                            var.name,
                            many.iter()
                                .map(|&m| var.values[m].as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    },
                })
                .collect()
        }
        Set::Ranges(domain) => {
            let low = domain[0].0;
            let mut starts: BTreeSet<i64> = BTreeSet::new();
            for set in atoms {
                if let Set::Ranges(ranges) = set {
                    for &(lo, hi) in ranges {
                        starts.insert(lo);
                        if let Some(next) = hi.checked_add(1) {
                            starts.insert(next);
                        }
                    }
                }
            }
            let mut starts: Vec<i64> = starts.into_iter().filter(|&s| s > low).collect();
            starts.insert(0, low);
            (0..starts.len())
                .map(|k| {
                    let lo = starts[k];
                    let hi = starts.get(k + 1).map_or(i64::MAX, |n| n - 1);
                    let label = if lo == low && hi == i64::MAX {
                        format!("{} has any value", var.name)
                    } else if lo == low {
                        format!("{} <= {}", var.name, shown(var, hi))
                    } else if hi == i64::MAX {
                        format!("{} >= {}", var.name, shown(var, lo))
                    } else if lo == hi {
                        format!("{} = {}", var.name, shown(var, lo))
                    } else {
                        format!(
                            "{} between {} and {}",
                            var.name,
                            shown(var, lo),
                            shown(var, hi)
                        )
                    };
                    Region {
                        rep: Rep::Int(lo),
                        label,
                    }
                })
                .collect()
        }
    }
}

/// The contexts for these scopes, or `None` when there are too many.
fn space(vocab: &Vocabulary, scopes: &[&Typed]) -> Option<Space> {
    let mut atoms: BTreeMap<&str, Vec<&Set>> = BTreeMap::new();
    for scope in scopes {
        scope.atoms(&mut atoms);
    }
    let mut vars = Vec::new();
    let mut all = Vec::new();
    let mut total: usize = 1;
    for (name, sets) in &atoms {
        let var = vocab.variables.get(*name)?;
        let found = regions(var, sets);
        total = total.saturating_mul(found.len());
        if total > MAX_CELLS {
            return None;
        }
        vars.push((*name).to_string());
        all.push(found);
    }
    let mut cells = vec![Vec::new()];
    for found in &all {
        cells = cells
            .into_iter()
            .flat_map(|cell| {
                (0..found.len()).map(move |k| {
                    let mut next = cell.clone();
                    next.push(k);
                    next
                })
            })
            .collect();
    }
    Some(Space {
        vars,
        regions: all,
        cells,
    })
}

// -------------------------------------------------------------- conflicts

/// Claim identity for de-duplicating what several cells report.
type Key = Vec<(String, usize)>;

fn key_of(claims: &[&Claim]) -> Key {
    let mut key: Key = claims.iter().map(|c| (c.node.clone(), c.line)).collect();
    key.sort();
    key
}

fn quoted(claims: &[&Claim]) -> String {
    claims
        .iter()
        .map(|c| format!("{} says \"{}\"", c.node, c.text))
        .collect::<Vec<_>>()
        .join(", ")
}

fn at_claim(finding: Finding, claim: &Claim) -> Finding {
    finding.at(&claim.path, Some(claim.line)).node(&claim.node)
}

/// Removes claims that another applicable claim overrides.
fn live<'a>(
    applicable: Vec<&'a Claim>,
    overrides: &BTreeMap<&str, BTreeSet<&str>>,
) -> Vec<&'a Claim> {
    if overrides.is_empty() {
        return applicable;
    }
    let mut beaten: HashSet<&str> = HashSet::new();
    for winner in &applicable {
        let Some(targets) = overrides.get(winner.node.as_str()) else {
            continue;
        };
        for &loser in targets {
            let mutual = overrides
                .get(loser)
                .is_some_and(|o| o.contains(winner.node.as_str()));
            if loser != winner.node && !mutual {
                beaten.insert(loser);
            }
        }
    }
    applicable
        .into_iter()
        .filter(|c| !beaten.contains(c.node.as_str()))
        .collect()
}

/// The smallest subset of `claims` that already has no common value, found
/// by dropping claims one at a time.
fn minimize<'a>(claims: &[&'a Claim], extra: Option<&Set>, domain: &Set) -> Vec<&'a Claim> {
    let mut core: Vec<&Claim> = claims.to_vec();
    let mut i = 0;
    while i < core.len() {
        let mut rest = extra.cloned().unwrap_or_else(|| domain.clone());
        for (k, c) in core.iter().enumerate() {
            if k != i {
                rest = rest.intersect(&c.allowed);
            }
        }
        if rest.is_empty() {
            core.remove(i);
        } else {
            i += 1;
        }
    }
    core
}

fn common(claims: &[&Claim], domain: &Set) -> Set {
    claims
        .iter()
        .fold(domain.clone(), |acc, c| acc.intersect(&c.allowed))
}

fn analyze_cells(
    subject: &str,
    claims: &[&Claim],
    space: &Space,
    domain: &Set,
    overrides: &BTreeMap<&str, BTreeSet<&str>>,
    seen: &mut HashSet<Key>,
    out: &mut Vec<Finding>,
) {
    let mut done: HashSet<Vec<usize>> = HashSet::new();
    for cell in &space.cells {
        let applicable: Vec<usize> = (0..claims.len())
            .filter(|&k| space.holds(cell, &claims[k].scope))
            .collect();
        if applicable.is_empty() || !done.insert(applicable.clone()) {
            continue;
        }
        let live = live(applicable.iter().map(|&k| claims[k]).collect(), overrides);
        let (soft, hard): (Vec<&Claim>, Vec<&Claim>) =
            live.into_iter().partition(|c| c.modality.is_soft());
        let allowed = common(&hard, domain);
        if allowed.is_empty() {
            let core = minimize(&hard, None, domain);
            if !seen.insert(key_of(&core)) {
                continue;
            }
            let first = core[0];
            let message = if let [only] = core.as_slice() {
                format!(
                    "{} asks for {subject} a value that does not exist: \"{}\"",
                    only.node, only.text
                )
            } else {
                format!(
                    "claims on {subject} conflict {}: {}. No value satisfies them all; add an `overrides` link, narrow a scope, or supersede one",
                    space.describe(cell, &core),
                    quoted(&core)
                )
            };
            out.push(at_claim(Finding::error("E019", message), first));
            continue;
        }
        for s in soft {
            if !common(&[s], &allowed).is_empty() {
                continue;
            }
            let mut core = minimize(&hard, Some(&s.allowed), domain);
            core.insert(0, s);
            if !seen.insert(key_of(&core)) {
                continue;
            }
            out.push(at_claim(
                Finding::warning(
                    "W005",
                    format!(
                        "a SHOULD claim on {subject} cannot hold {}: {}",
                        space.describe(cell, &core),
                        quoted(&core)
                    ),
                ),
                s,
            ));
        }
    }
}

fn check_variable(
    vocab: &Vocabulary,
    subject: &str,
    claims: &[&Claim],
    overrides: &BTreeMap<&str, BTreeSet<&str>>,
    out: &mut Vec<Finding>,
) {
    let domain = vocab.variables[subject].domain();
    let scopes: Vec<&Typed> = claims.iter().filter_map(|c| c.scope.as_ref()).collect();
    let mut seen = HashSet::new();
    let Some(all) = space(vocab, &scopes) else {
        // Too many contexts at once: look at pairs, which finds what two claims do.
        for (i, &a) in claims.iter().enumerate() {
            for &b in &claims[i + 1..] {
                let pair = [a, b];
                // Two claims only conflict if no value satisfies both.
                if !common(&pair, &domain).is_empty() {
                    continue;
                }
                let pair_scopes: Vec<&Typed> =
                    pair.iter().filter_map(|c| c.scope.as_ref()).collect();
                if let Some(contexts) = space(vocab, &pair_scopes) {
                    analyze_cells(
                        subject, &pair, &contexts, &domain, overrides, &mut seen, out,
                    );
                }
            }
        }
        return;
    };
    analyze_cells(subject, claims, &all, &domain, overrides, &mut seen, out);

    // Claims that can never apply.
    for claim in claims {
        let Some(scope) = &claim.scope else { continue };
        let Some(own) = space(vocab, &[scope]) else {
            continue;
        };
        if !own.cells.iter().any(|cell| own.holds(cell, &claim.scope)) {
            out.push(at_claim(
                Finding::warning(
                    "W006",
                    format!(
                        "a claim on {subject} never applies, because its scope can never hold: \"{}\"",
                        claim.text
                    ),
                ),
                claim,
            ));
        }
    }

    // Claims that say nothing another claim has not already said.
    if claims.len() <= MAX_FOR_REDUNDANCY {
        let hard: Vec<&Claim> = claims
            .iter()
            .copied()
            .filter(|c| !c.modality.is_soft())
            .collect();
        for &later in &hard {
            let implied_by = hard.iter().copied().find(|&earlier| {
                !std::ptr::eq(earlier, later)
                    && earlier.allowed.is_subset_of(&later.allowed)
                    && all.cells.iter().all(|cell| {
                        !all.holds(cell, &later.scope) || all.holds(cell, &earlier.scope)
                    })
                    // Identical claims are reported on the later one only.
                    && !(same_meaning(&all, earlier, later)
                        && (later.node.as_str(), later.line)
                            < (earlier.node.as_str(), earlier.line))
            });
            if let Some(earlier) = implied_by {
                out.push(at_claim(
                    Finding::info(
                        "I002",
                        format!(
                            "a claim on {subject} is already implied by {}: \"{}\"",
                            earlier.node, earlier.text
                        ),
                    ),
                    later,
                ));
            }
        }
    }
}

/// Two claims that apply in the same cells and allow the same values.
fn same_meaning(space: &Space, a: &Claim, b: &Claim) -> bool {
    a.allowed == b.allowed
        && space
            .cells
            .iter()
            .all(|cell| space.holds(cell, &a.scope) == space.holds(cell, &b.scope))
}

fn overrides_of(graph: &Graph) -> BTreeMap<&str, BTreeSet<&str>> {
    let mut map: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for node in &graph.nodes {
        for target in node.links.get("overrides").into_iter().flatten() {
            map.entry(node.id.as_str())
                .or_default()
                .insert(id::base_of(target));
        }
    }
    map
}

/// Conflicts and lints over the claims that count.
pub fn conflicts(graph: &Graph, analysis: &Analysis) -> Vec<Finding> {
    let overrides = overrides_of(graph);
    let mut by_subject: BTreeMap<&str, Vec<&Claim>> = BTreeMap::new();
    for claim in analysis.claims.iter().filter(|c| c.active) {
        by_subject.entry(&claim.subject).or_default().push(claim);
    }
    let mut out = Vec::new();
    for (subject, mut claims) in by_subject {
        claims.sort_by(|a, b| (&a.node, a.line).cmp(&(&b.node, b.line)));
        check_variable(&analysis.vocabulary, subject, &claims, &overrides, &mut out);
    }
    out
}

/// All findings from claims: unreadable ones, then conflicts and lints.
pub fn check(graph: &Graph) -> Vec<Finding> {
    let analysis = analyze(graph);
    let mut findings = analysis.findings.clone();
    findings.extend(conflicts(graph, &analysis));
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Graph;
    use crate::node::Node;
    use crate::parse::parse_file_text;
    use std::path::Path;

    fn node(path: &str, text: &str) -> Node {
        let (node, findings) = parse_file_text(Path::new(path), text);
        assert!(findings.is_empty(), "{findings:?}");
        node.expect("a node")
    }

    const VOCAB: &str = "---\nid: VOC-0001\ntype: vocabulary\n---\n# Words\n\n```vocabulary\nsession.idle_timeout: {type: duration}\npassword.min_length: {type: int}\nrole: {type: enum, values: [admin, user, guest]}\nenv: {type: enum, values: [dev, prod]}\nmfa: {type: bool}\n```\n";

    fn requirement(id: &str, status: &str, links: &str, claims: &str) -> String {
        format!(
            "---\nid: {id}\ntype: requirement\nstatus: {status}\n{links}---\n### Requirement: R\nIt MUST.\n\n#### Scenario: S {{#S1}}\n- **WHEN** a\n- **THEN** b\n\n```claims\n{claims}```\n"
        )
    }

    fn graph(requirements: &[(&str, &str, &str, &str)]) -> Graph {
        let mut nodes = vec![node("vocab.md", VOCAB)];
        for (i, (id, status, links, claims)) in requirements.iter().enumerate() {
            nodes.push(node(
                &format!("r{i}.md"),
                &requirement(id, status, links, claims),
            ));
        }
        Graph::from_nodes(nodes)
    }

    fn codes(findings: &[Finding]) -> Vec<&'static str> {
        findings.iter().map(|f| f.code).collect()
    }

    /// Codes of everything but the informational notes.
    fn problems(findings: &[Finding]) -> Vec<&'static str> {
        findings
            .iter()
            .filter(|f| f.severity != crate::finding::Severity::Info)
            .map(|f| f.code)
            .collect()
    }

    #[test]
    fn durations_have_units() {
        assert_eq!(format_duration(900_000), "15m");
        assert_eq!(format_duration(1_800_000), "30m");
        assert_eq!(format_duration(90_000), "90s");
        assert_eq!(format_duration(1_500), "1500ms");
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(172_800_000), "2d");
    }

    #[test]
    fn range_sets_follow_the_algebra() {
        let a = Set::Ranges(vec![(1, 5), (10, 20)]);
        let b = Set::Ranges(vec![(4, 12)]);
        assert_eq!(a.intersect(&b), Set::Ranges(vec![(4, 5), (10, 12)]));
        assert_eq!(a.union(&b), Set::Ranges(vec![(1, 20)]));
        let domain = Set::Ranges(vec![(0, i64::MAX)]);
        assert_eq!(
            a.complement_in(&domain),
            Set::Ranges(vec![(0, 0), (6, 9), (21, i64::MAX)])
        );
        assert!(Set::Ranges(vec![(2, 3)]).is_subset_of(&a));
        assert!(!b.is_subset_of(&a));
        // Neighbouring integers join, and the ends do not overflow.
        assert_eq!(
            Set::Ranges(vec![(1, 2)]).union(&Set::Ranges(vec![(3, 4)])),
            Set::Ranges(vec![(1, 4)])
        );
        let all = Set::Ranges(vec![(i64::MIN, i64::MAX)]);
        assert!(all.complement_in(&all).is_empty());
    }

    #[test]
    fn the_admin_timeout_example_conflicts() {
        let g = graph(&[
            (
                "REQ-AUTH-0004",
                "accepted",
                "",
                "for *: session.idle_timeout == 30m\n",
            ),
            (
                "REQ-AUTH-0012",
                "accepted",
                "",
                "for role = admin: session.idle_timeout <= 15m\n",
            ),
        ]);
        let findings = check(&g);
        assert_eq!(codes(&findings), ["E019"], "{findings:?}");
        let f = &findings[0];
        assert!(f.message.contains("when role = admin"), "{}", f.message);
        assert!(f.message.contains("REQ-AUTH-0004"));
        assert!(f.message.contains("REQ-AUTH-0012"));
        assert_eq!(f.id.as_deref(), Some("REQ-AUTH-0004"));
        // The message does not carry line numbers, so editing above the claim
        // does not make the conflict look new to `oxispec ci`.
        assert!(!f.message.contains("line"));
    }

    #[test]
    fn claims_that_never_meet_do_not_conflict() {
        let g = graph(&[
            (
                "REQ-A-0001",
                "accepted",
                "",
                "for role = user: session.idle_timeout == 30m\n",
            ),
            (
                "REQ-A-0002",
                "accepted",
                "",
                "for role = admin: session.idle_timeout <= 15m\n",
            ),
        ]);
        assert!(check(&g).is_empty());
    }

    #[test]
    fn a_broad_scope_still_meets_a_narrow_one() {
        // `role != admin` includes user and guest, so it meets `role = guest`.
        let g = graph(&[
            (
                "REQ-A-0001",
                "accepted",
                "",
                "for role != admin: password.min_length >= 12\n",
            ),
            (
                "REQ-A-0002",
                "accepted",
                "",
                "for role = guest: password.min_length <= 8\n",
            ),
        ]);
        let findings = check(&g);
        assert_eq!(codes(&findings), ["E019"], "{findings:?}");
        assert!(findings[0].message.contains("role = guest"));
    }

    #[test]
    fn numeric_scopes_split_at_their_edges() {
        let g = graph(&[
            (
                "REQ-A-0001",
                "accepted",
                "",
                "for password.min_length >= 10: session.idle_timeout <= 5m\n",
            ),
            (
                "REQ-A-0002",
                "accepted",
                "",
                "for password.min_length < 10: session.idle_timeout >= 1h\n",
            ),
            (
                "REQ-A-0003",
                "accepted",
                "",
                "for password.min_length <= 10: session.idle_timeout >= 1h\n",
            ),
        ]);
        let findings = check(&g);
        // 0001 and 0003 meet at exactly 10, and 0001 and 0002 never meet.
        assert_eq!(problems(&findings), ["E019"], "{findings:?}");
        assert!(findings[0].message.contains("REQ-A-0003"));
        assert!(!findings[0].message.contains("REQ-A-0002"));
        assert!(
            findings[0].message.contains("password.min_length = 10"),
            "{}",
            findings[0].message
        );
    }

    #[test]
    fn a_core_is_as_small_as_it_can_be() {
        // Three claims, but only two of them cannot hold together.
        let g = graph(&[
            ("REQ-A-0001", "accepted", "", "session.idle_timeout <= 1h\n"),
            ("REQ-A-0002", "accepted", "", "session.idle_timeout >= 2h\n"),
            ("REQ-A-0003", "accepted", "", "session.idle_timeout != 5m\n"),
        ]);
        let findings = check(&g);
        assert_eq!(problems(&findings), ["E019"], "{findings:?}");
        assert!(!findings[0].message.contains("REQ-A-0003"));
        assert!(findings[0].message.contains("in every context"));
    }

    #[test]
    fn three_claims_can_conflict_when_no_two_do() {
        let g = graph(&[
            ("REQ-A-0001", "accepted", "", "role in [admin, user]\n"),
            ("REQ-A-0002", "accepted", "", "role in [user, guest]\n"),
            ("REQ-A-0003", "accepted", "", "role in [admin, guest]\n"),
        ]);
        let findings = check(&g);
        assert_eq!(codes(&findings), ["E019"], "{findings:?}");
        for id in ["REQ-A-0001", "REQ-A-0002", "REQ-A-0003"] {
            assert!(findings[0].message.contains(id), "{}", findings[0].message);
        }
    }

    #[test]
    fn must_not_and_booleans() {
        let g = graph(&[
            ("REQ-A-0001", "accepted", "", "for env = prod: mfa\n"),
            (
                "REQ-A-0002",
                "accepted",
                "",
                "must not for env = prod: mfa\n",
            ),
            ("REQ-A-0003", "accepted", "", "for env = dev: not mfa\n"),
        ]);
        let findings = check(&g);
        assert_eq!(codes(&findings), ["E019"], "{findings:?}");
        assert!(findings[0].message.contains("env = prod"));
        assert!(!findings[0].message.contains("REQ-A-0003"));
    }

    #[test]
    fn an_overrides_link_makes_a_conflict_intentional() {
        let claims = [
            (
                "REQ-AUTH-0004",
                "accepted",
                "",
                "for *: session.idle_timeout == 30m\n",
            ),
            (
                "REQ-AUTH-0012",
                "accepted",
                "links:\n  overrides: [REQ-AUTH-0004]\n",
                "for role = admin: session.idle_timeout <= 15m\n",
            ),
        ];
        assert!(check(&graph(&claims)).is_empty());
        // The other way round, the broad claim would win over the narrow one.
        let wrong = [
            claims[0],
            (
                "REQ-AUTH-0012",
                "accepted",
                "links:\n  overrides: [REQ-AUTH-0004]\n",
                "for role = admin: session.idle_timeout <= 15m\n",
            ),
        ];
        assert!(check(&graph(&wrong)).is_empty());
    }

    #[test]
    fn overriding_does_not_hide_a_third_claim() {
        let g = graph(&[
            (
                "REQ-A-0001",
                "accepted",
                "",
                "session.idle_timeout == 30m\n",
            ),
            (
                "REQ-A-0002",
                "accepted",
                "links:\n  overrides: [REQ-A-0001]\n",
                "session.idle_timeout <= 15m\n",
            ),
            (
                "REQ-A-0003",
                "accepted",
                "",
                "session.idle_timeout >= 20m\n",
            ),
        ]);
        let findings = check(&g);
        assert_eq!(problems(&findings), ["E019"], "{findings:?}");
        assert!(findings[0].message.contains("REQ-A-0002"));
        assert!(findings[0].message.contains("REQ-A-0003"));
        assert!(!findings[0].message.contains("REQ-A-0001"));
    }

    #[test]
    fn rejected_superseded_and_withdrawn_nodes_do_not_count() {
        for status in ["rejected", "superseded", "deprecated", "withdrawn"] {
            let g = graph(&[
                (
                    "REQ-A-0001",
                    "accepted",
                    "",
                    "session.idle_timeout == 30m\n",
                ),
                ("REQ-A-0002", status, "", "session.idle_timeout <= 15m\n"),
            ]);
            assert!(check(&g).is_empty(), "{status}");
        }
        // Draft and proposed do: that is when a conflict is cheapest to fix.
        for status in ["draft", "proposed"] {
            let g = graph(&[
                (
                    "REQ-A-0001",
                    "accepted",
                    "",
                    "session.idle_timeout == 30m\n",
                ),
                ("REQ-A-0002", status, "", "session.idle_timeout <= 15m\n"),
            ]);
            assert_eq!(codes(&check(&g)), ["E019"], "{status}");
        }
        // A node that another node supersedes does not count either.
        let g = graph(&[
            (
                "REQ-A-0001",
                "accepted",
                "",
                "session.idle_timeout == 30m\n",
            ),
            (
                "REQ-A-0002",
                "accepted",
                "links:\n  supersedes: [REQ-A-0001]\n",
                "session.idle_timeout <= 15m\n",
            ),
        ]);
        assert!(check(&g).is_empty());
    }

    #[test]
    fn a_claim_that_asks_for_nothing_is_an_error() {
        let g = graph(&[("REQ-A-0001", "accepted", "", "session.idle_timeout < 0s\n")]);
        let findings = check(&g);
        assert_eq!(codes(&findings), ["E019"], "{findings:?}");
        assert!(findings[0].message.contains("a value that does not exist"));
    }

    #[test]
    fn should_claims_only_warn() {
        let g = graph(&[
            (
                "REQ-A-0001",
                "accepted",
                "",
                "session.idle_timeout <= 15m\n",
            ),
            (
                "REQ-A-0002",
                "accepted",
                "",
                "should session.idle_timeout >= 1h\n",
            ),
        ]);
        let findings = check(&g);
        assert_eq!(codes(&findings), ["W005"], "{findings:?}");
        assert_eq!(findings[0].id.as_deref(), Some("REQ-A-0002"));
        assert!(findings[0].message.contains("REQ-A-0001"));
    }

    #[test]
    fn a_scope_that_cannot_hold_is_a_dead_claim() {
        let g = graph(&[(
            "REQ-A-0001",
            "accepted",
            "",
            "for role = admin and role = guest: mfa\n",
        )]);
        assert_eq!(codes(&check(&g)), ["W006"]);
    }

    #[test]
    fn an_implied_claim_is_reported_once() {
        let g = graph(&[
            (
                "REQ-A-0001",
                "accepted",
                "",
                "for *: session.idle_timeout <= 15m\n",
            ),
            (
                "REQ-A-0002",
                "accepted",
                "",
                "for role = admin: session.idle_timeout <= 30m\n",
            ),
        ]);
        let findings = check(&g);
        assert_eq!(codes(&findings), ["I002"], "{findings:?}");
        assert_eq!(findings[0].id.as_deref(), Some("REQ-A-0002"));
        // The specific claim is not implied by the general one, and the
        // general claim is not implied by the specific one.
        let g = graph(&[
            (
                "REQ-A-0001",
                "accepted",
                "",
                "for *: session.idle_timeout <= 30m\n",
            ),
            (
                "REQ-A-0002",
                "accepted",
                "",
                "for role = admin: session.idle_timeout <= 15m\n",
            ),
        ]);
        assert!(check(&g).is_empty());
        // The same claim twice is reported on the later one only.
        let g = graph(&[
            (
                "REQ-A-0001",
                "accepted",
                "",
                "session.idle_timeout <= 15m\n",
            ),
            (
                "REQ-A-0002",
                "accepted",
                "",
                "session.idle_timeout <= 15m\n",
            ),
        ]);
        let findings = check(&g);
        assert_eq!(codes(&findings), ["I002"], "{findings:?}");
        assert_eq!(findings[0].id.as_deref(), Some("REQ-A-0002"));
    }

    #[test]
    fn too_many_contexts_fall_back_to_pairs() {
        // 13 independent scope variables make 2^13 contexts for one subject.
        let mut vocab = String::from(
            "---\nid: VOC-0002\ntype: vocabulary\n---\n```vocabulary\nlimit: {type: int}\n",
        );
        for i in 0..13 {
            vocab.push_str(&format!("flag{i}: {{type: bool}}\n"));
        }
        vocab.push_str("```\n");
        let mut nodes = vec![node("v.md", &vocab)];
        for i in 0..13 {
            nodes.push(node(
                &format!("r{i}.md"),
                &requirement(
                    &format!("REQ-A-{i:04}"),
                    "accepted",
                    "",
                    &format!("for flag{i}: limit != {i}\n"),
                ),
            ));
        }
        nodes.push(node(
            "x.md",
            &requirement("REQ-A-0100", "accepted", "", "for flag3: limit == 3\n"),
        ));
        let findings = check(&Graph::from_nodes(nodes));
        assert_eq!(problems(&findings), ["E019"], "{findings:?}");
        let message = &findings[0].message;
        assert!(message.contains("REQ-A-0003") && message.contains("REQ-A-0100"));
        assert!(message.contains("flag3 = true"), "{message}");
    }

    #[test]
    fn unreadable_claims_are_reported_with_the_reason() {
        let cases = [
            ("session.idle_timeout <= 15", "write a unit"),
            ("password.min_length <= 5m", "without a unit"),
            ("sesion.idle_timeout <= 15m", "unknown variable"),
            ("role = root", "not a value of `role`"),
            ("role < admin", "cannot be ordered"),
            ("session.idle_timeout <= 15x", "unknown unit"),
            (
                "for role = admin session.idle_timeout <= 15m",
                "expected `:`",
            ),
            ("for role = admin:", "no constraint"),
            ("session.idle_timeout <= 15m and mfa", "one variable"),
            ("for mfa: mfa", "cannot use `mfa`"),
            ("session.idle_timeout <=", "ends where a value"),
            ("session.idle_timeout <= 15m 3", "after the constraint"),
            ("role", "is an enum"),
            ("mfa = 3", "not a number"),
            ("role = admin $", "unexpected `$`"),
        ];
        for (text, reason) in cases {
            let g = graph(&[("REQ-A-0001", "accepted", "", &format!("{text}\n"))]);
            let findings = check(&g);
            assert_eq!(codes(&findings), ["E017"], "{text}: {findings:?}");
            assert!(
                findings[0].message.contains(reason),
                "{text}: {}",
                findings[0].message
            );
            assert_eq!(findings[0].id.as_deref(), Some("REQ-A-0001"));
        }
    }

    #[test]
    fn the_vocabulary_is_checked() {
        let bad = |block: &str| {
            let text =
                format!("---\nid: VOC-0009\ntype: vocabulary\n---\n```vocabulary\n{block}```\n");
            let g = Graph::from_nodes(vec![node("v.md", &text)]);
            check(&g)
        };
        for block in [
            "x: {type: float}\n",
            "x: {type: enum}\n",
            "x: {type: enum, values: []}\n",
            "x: {type: enum, values: [a, a]}\n",
            "x: {type: enum, values: [for]}\n",
            "x: {type: int, values: [a]}\n",
            "x: {type: bool, values: [a]}\n",
            "x: {type: int, colour: red}\n",
            "for: {type: int}\n",
            "a-b: {type: int}\n",
            "- not a map\n",
        ] {
            assert_eq!(codes(&bad(block)), ["E018"], "{block}");
        }
        assert!(bad("# nothing yet\n").is_empty());
        // A name declared in two nodes.
        let twice = Graph::from_nodes(vec![
            node("a.md", VOCAB),
            node(
                "b.md",
                "---\nid: VOC-0002\ntype: vocabulary\n---\n```vocabulary\nrole: {type: int}\n```\n",
            ),
        ]);
        let findings = check(&twice);
        assert_eq!(codes(&findings), ["E018"]);
        assert!(findings[0].message.contains("first in VOC-0001"));
    }

    #[test]
    fn a_project_without_claims_costs_nothing() {
        let g = Graph::from_nodes(vec![node(
            "r.md",
            "---\nid: REQ-A-0001\ntype: requirement\n---\n### Requirement: R\nIt MUST.\n",
        )]);
        let analysis = analyze(&g);
        assert!(analysis.claims.is_empty() && analysis.findings.is_empty());
        assert!(check(&g).is_empty());
    }

    #[test]
    fn keywords_are_not_case_sensitive_and_comments_are_skipped() {
        let g = graph(&[(
            "REQ-A-0001",
            "accepted",
            "",
            "# a note\n\nFOR ROLE_X: mfa\n",
        )]);
        // `ROLE_X` is not declared, `FOR` is read as a keyword.
        let findings = check(&g);
        assert_eq!(codes(&findings), ["E017"]);
        assert!(findings[0].message.contains("unknown variable `ROLE_X`"));
        let g = graph(&[(
            "REQ-A-0001",
            "accepted",
            "",
            "MUST NOT FOR env = prod AND role IN [guest]: mfa\n",
        )]);
        assert!(check(&g).is_empty());
    }
}
