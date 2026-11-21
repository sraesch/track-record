//! Units (FR-13a).
//!
//! Units are stored as symbols. The predefined catalogue (the `unit` table)
//! gives every known symbol a quantity and a factor to the base unit of that
//! quantity. Symbols outside the catalogue are custom units: they are allowed
//! everywhere but only convert to themselves.
//!
//! For derived metrics, units are combined with `*`, `/` and integer powers,
//! e.g. `B/s` or `MB*s`. Every quantity of the catalogue maps to a dimension
//! (a product of base dimensions with integer exponents), so `MB / s` and
//! `kB/s` are recognised as the same dimension and convert with a factor.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

/// One unit of the predefined catalogue.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogueUnit {
    pub symbol: String,
    pub quantity: String,
    /// Value in this unit times `to_base` is the value in the base unit.
    pub to_base: f64,
    pub name: String,
}

/// Product of base dimensions with integer exponents. Empty means
/// dimensionless.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Dimension(BTreeMap<String, i32>);

impl Dimension {
    pub fn dimensionless() -> Self {
        Self::default()
    }

    pub fn base(name: &str) -> Self {
        let mut map = BTreeMap::new();
        map.insert(name.to_owned(), 1);
        Self(map)
    }

    fn from_pairs(pairs: &[(&str, i32)]) -> Self {
        let mut d = Self::default();
        for (name, exp) in pairs {
            d.add(name, *exp);
        }
        d
    }

    fn add(&mut self, name: &str, exp: i32) {
        let entry = self.0.entry(name.to_owned()).or_insert(0);
        *entry += exp;
        if *entry == 0 {
            self.0.remove(name);
        }
    }

    pub fn is_dimensionless(&self) -> bool {
        self.0.is_empty()
    }

    pub fn mul(&self, other: &Self) -> Self {
        let mut d = self.clone();
        for (name, exp) in &other.0 {
            d.add(name, *exp);
        }
        d
    }

    pub fn pow(&self, exp: i32) -> Self {
        let mut d = Self::default();
        for (name, e) in &self.0 {
            d.add(name, e * exp);
        }
        d
    }

    pub fn inverse(&self) -> Self {
        self.pow(-1)
    }
}

/// Dimension of a catalogue quantity. Quantities unknown here are treated as
/// base dimensions of their own, so the catalogue can grow through migrations
/// without a code change (they just don't combine with other quantities).
pub fn quantity_dimension(quantity: &str) -> Dimension {
    match quantity {
        "time" => Dimension::from_pairs(&[("time", 1)]),
        "data size" => Dimension::from_pairs(&[("data", 1)]),
        "data rate" => Dimension::from_pairs(&[("data", 1), ("time", -1)]),
        "frequency" => Dimension::from_pairs(&[("time", -1)]),
        "frame rate" => Dimension::from_pairs(&[("frames", 1), ("time", -1)]),
        "ratio" => Dimension::dimensionless(),
        "count" => Dimension::from_pairs(&[("count", 1)]),
        "length" => Dimension::from_pairs(&[("length", 1)]),
        "pixels" => Dimension::from_pairs(&[("pixels", 1)]),
        "power" => Dimension::from_pairs(&[("energy", 1), ("time", -1)]),
        "energy" => Dimension::from_pairs(&[("energy", 1)]),
        other => Dimension::base(&format!("quantity:{other}")),
    }
}

/// A unit expression resolved against the catalogue: its dimension and the
/// factor that converts a value in this unit to the base units.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedUnit {
    pub symbol: String,
    pub dimension: Dimension,
    pub to_base: f64,
}

impl ResolvedUnit {
    pub fn dimensionless() -> Self {
        Self {
            symbol: String::new(),
            dimension: Dimension::dimensionless(),
            to_base: 1.0,
        }
    }

    /// Factor that converts a value in `self` to `other` (value * factor), or
    /// `None` if the dimensions differ.
    pub fn conversion_factor(&self, other: &ResolvedUnit) -> Option<f64> {
        if self.symbol == other.symbol {
            return Some(1.0);
        }
        (self.dimension == other.dimension).then(|| self.to_base / other.to_base)
    }

    pub fn mul(&self, other: &ResolvedUnit) -> ResolvedUnit {
        ResolvedUnit {
            symbol: combine(&self.symbol, '*', &other.symbol),
            dimension: self.dimension.mul(&other.dimension),
            to_base: self.to_base * other.to_base,
        }
    }

    pub fn div(&self, other: &ResolvedUnit) -> ResolvedUnit {
        ResolvedUnit {
            symbol: combine(&self.symbol, '/', &other.symbol),
            dimension: self.dimension.mul(&other.dimension.inverse()),
            to_base: self.to_base / other.to_base,
        }
    }

    pub fn powi(&self, exp: i32) -> ResolvedUnit {
        let symbol = match (exp, self.symbol.is_empty()) {
            (_, true) | (0, _) => String::new(),
            (1, _) => self.symbol.clone(),
            _ => format!("({})^{exp}", self.symbol),
        };
        ResolvedUnit {
            symbol,
            dimension: self.dimension.pow(exp),
            to_base: self.to_base.powi(exp),
        }
    }
}

fn combine(a: &str, op: char, b: &str) -> String {
    let wrap = |s: &str| {
        if s.contains(['*', '/']) {
            format!("({s})")
        } else {
            s.to_owned()
        }
    };
    match (a.is_empty(), b.is_empty(), op) {
        (_, true, _) => a.to_owned(),
        (true, false, '*') => b.to_owned(),
        (true, false, _) => format!("1/{}", wrap(b)),
        (false, false, _) => format!("{a}{op}{}", wrap(b)),
    }
}

impl fmt::Display for ResolvedUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.symbol)
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum UnitError {
    #[error("invalid unit expression {0:?}: {1}")]
    Syntax(String, &'static str),
}

/// The predefined unit catalogue, loaded from the database.
#[derive(Debug, Clone, Default)]
pub struct UnitCatalogue {
    units: HashMap<String, CatalogueUnit>,
}

impl UnitCatalogue {
    pub fn new(units: impl IntoIterator<Item = CatalogueUnit>) -> Self {
        Self {
            units: units.into_iter().map(|u| (u.symbol.clone(), u)).collect(),
        }
    }

    pub fn get(&self, symbol: &str) -> Option<&CatalogueUnit> {
        self.units.get(symbol)
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    /// Resolves a single symbol. Symbols outside the catalogue are custom
    /// units: a base dimension of their own with factor 1.
    pub fn resolve_symbol(&self, symbol: &str) -> ResolvedUnit {
        match self.units.get(symbol) {
            Some(u) => ResolvedUnit {
                symbol: symbol.to_owned(),
                dimension: quantity_dimension(&u.quantity),
                to_base: u.to_base,
            },
            None if symbol.is_empty() => ResolvedUnit::dimensionless(),
            None => ResolvedUnit {
                symbol: symbol.to_owned(),
                dimension: Dimension::base(&format!("custom:{symbol}")),
                to_base: 1.0,
            },
        }
    }

    /// Resolves a unit expression. A symbol from the catalogue (including
    /// ones that contain `/`, such as `B/s`) is taken as is. Otherwise the
    /// expression is parsed as factors joined by `*` and `/` (each `/`
    /// divides by the next factor only), with optional integer powers `^n`
    /// and parentheses. A string that does not parse is a custom unit.
    pub fn resolve(&self, expr: &str) -> ResolvedUnit {
        let expr = expr.trim();
        if self.units.contains_key(expr) || expr.is_empty() {
            return self.resolve_symbol(expr);
        }
        match UnitParser::new(self, expr).parse() {
            Ok(mut unit) => {
                unit.symbol = expr.to_owned();
                unit
            }
            Err(_) => self.resolve_symbol(expr),
        }
    }

    /// Factor converting a value in unit `from` to unit `to`, or `None` if
    /// they cannot be converted.
    pub fn conversion_factor(&self, from: &str, to: &str) -> Option<f64> {
        self.resolve(from).conversion_factor(&self.resolve(to))
    }

    /// True if both units have the same dimension.
    pub fn convertible(&self, from: &str, to: &str) -> bool {
        self.conversion_factor(from, to).is_some()
    }
}

struct UnitParser<'a> {
    catalogue: &'a UnitCatalogue,
    input: &'a str,
    pos: usize,
}

impl<'a> UnitParser<'a> {
    fn new(catalogue: &'a UnitCatalogue, input: &'a str) -> Self {
        Self {
            catalogue,
            input,
            pos: 0,
        }
    }

    fn err(&self, msg: &'static str) -> UnitError {
        UnitError::Syntax(self.input.to_owned(), msg)
    }

    fn rest(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn skip_ws(&mut self) {
        let trimmed = self.rest().trim_start();
        self.pos = self.input.len() - trimmed.len();
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_ws();
        self.rest().chars().next()
    }

    fn bump(&mut self) {
        if let Some(c) = self.rest().chars().next() {
            self.pos += c.len_utf8();
        }
    }

    fn parse(mut self) -> Result<ResolvedUnit, UnitError> {
        let unit = self.product()?;
        if self.peek().is_some() {
            return Err(self.err("unexpected character"));
        }
        Ok(unit)
    }

    fn product(&mut self) -> Result<ResolvedUnit, UnitError> {
        let mut unit = self.power()?;
        loop {
            match self.peek() {
                Some('*' | '·') => {
                    self.bump();
                    unit = unit.mul(&self.power()?);
                }
                Some('/') => {
                    self.bump();
                    unit = unit.div(&self.power()?);
                }
                _ => return Ok(unit),
            }
        }
    }

    fn power(&mut self) -> Result<ResolvedUnit, UnitError> {
        let base = self.atom()?;
        if self.peek() == Some('^') {
            self.bump();
            self.skip_ws();
            let start = self.pos;
            if self.rest().starts_with('-') {
                self.bump();
            }
            while self.rest().starts_with(|c: char| c.is_ascii_digit()) {
                self.bump();
            }
            let exp: i32 = self.input[start..self.pos]
                .parse()
                .map_err(|_| self.err("invalid exponent"))?;
            return Ok(base.powi(exp));
        }
        Ok(base)
    }

    fn atom(&mut self) -> Result<ResolvedUnit, UnitError> {
        match self.peek() {
            Some('(') => {
                self.bump();
                let inner = self.product()?;
                if self.peek() != Some(')') {
                    return Err(self.err("missing ')'"));
                }
                self.bump();
                Ok(inner)
            }
            Some('1') => {
                self.bump();
                Ok(ResolvedUnit::dimensionless())
            }
            Some(_) => {
                let start = self.pos;
                while let Some(c) = self.rest().chars().next() {
                    if matches!(c, '*' | '·' | '/' | '^' | '(' | ')') || c.is_whitespace() {
                        break;
                    }
                    self.bump();
                }
                if start == self.pos {
                    return Err(self.err("expected a unit symbol"));
                }
                Ok(self.catalogue.resolve_symbol(&self.input[start..self.pos]))
            }
            None => Err(self.err("expected a unit symbol")),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A slice of the catalogue from migration 0010.
    pub(crate) fn catalogue() -> UnitCatalogue {
        let rows: &[(&str, &str, f64)] = &[
            ("time", "ms", 1e-3),
            ("time", "s", 1.0),
            ("time", "min", 60.0),
            ("data size", "B", 1.0),
            ("data size", "kB", 1e3),
            ("data size", "MB", 1e6),
            ("data size", "bit", 0.125),
            ("data rate", "B/s", 1.0),
            ("data rate", "MB/s", 1e6),
            ("data rate", "Mbit/s", 125000.0),
            ("frequency", "Hz", 1.0),
            ("frame rate", "fps", 1.0),
            ("ratio", "", 1.0),
            ("ratio", "%", 1e-2),
            ("power", "W", 1.0),
            ("energy", "J", 1.0),
            ("energy", "Wh", 3600.0),
        ];
        UnitCatalogue::new(rows.iter().map(|(q, s, f)| CatalogueUnit {
            symbol: (*s).to_owned(),
            quantity: (*q).to_owned(),
            to_base: *f,
            name: (*s).to_owned(),
        }))
    }

    fn approx(a: Option<f64>, b: f64) -> bool {
        a.is_some_and(|a| (a - b).abs() <= 1e-12 * b.abs().max(1.0))
    }

    #[test]
    fn converts_within_a_quantity() {
        let c = catalogue();
        assert!(approx(c.conversion_factor("ms", "s"), 1e-3));
        assert!(approx(c.conversion_factor("min", "ms"), 60_000.0));
        assert!(approx(c.conversion_factor("MB/s", "Mbit/s"), 8.0));
        assert!(approx(c.conversion_factor("%", ""), 0.01));
        assert_eq!(c.conversion_factor("s", "B"), None);
    }

    #[test]
    fn composite_units_match_catalogue_quantities() {
        let c = catalogue();
        assert!(approx(c.conversion_factor("MB / s", "B/s"), 1e6));
        assert!(approx(c.conversion_factor("kB/ms", "MB/s"), 1.0));
        assert!(approx(c.conversion_factor("1/s", "Hz"), 1.0));
        assert!(approx(c.conversion_factor("W*s", "J"), 1.0));
        assert!(approx(c.conversion_factor("Wh", "W*min"), 60.0));
        assert!(approx(c.conversion_factor("B/s^2*s", "B/s"), 1.0));
        assert!(approx(c.conversion_factor("(B/s)*s", "B"), 1.0));
        assert!(approx(c.conversion_factor("s^-1", "Hz"), 1.0));
    }

    #[test]
    fn frame_rate_is_not_frequency() {
        let c = catalogue();
        assert_eq!(c.conversion_factor("fps", "Hz"), None);
    }

    #[test]
    fn custom_units_only_convert_to_themselves() {
        let c = catalogue();
        assert!(approx(c.conversion_factor("triangles", "triangles"), 1.0));
        assert_eq!(c.conversion_factor("triangles", "count"), None);
        // Composite with a custom unit keeps the custom dimension.
        assert!(approx(
            c.conversion_factor("triangles/ms", "triangles/s"),
            1000.0
        ));
        assert_eq!(c.conversion_factor("°C", "K"), None);
    }

    #[test]
    fn unparsable_expressions_are_custom_units() {
        let c = catalogue();
        let u = c.resolve("a/(b");
        assert_eq!(u.symbol, "a/(b");
        assert!(approx(c.conversion_factor("a/(b", "a/(b"), 1.0));
        assert_eq!(c.conversion_factor("a/(b", "a"), None);
    }

    #[test]
    fn unit_algebra_builds_symbols() {
        let c = catalogue();
        let b = c.resolve("B");
        let s = c.resolve("s");
        let rate = b.div(&s);
        assert_eq!(rate.symbol, "B/s");
        assert!(approx(rate.conversion_factor(&c.resolve("MB/s")), 1e-6));
        assert_eq!(rate.mul(&s).dimension, b.dimension);
        assert_eq!(ResolvedUnit::dimensionless().div(&s).symbol, "1/s");
        assert_eq!(b.div(&rate).symbol, "B/(B/s)");
        assert!(rate.powi(0).dimension.is_dimensionless());
    }

    #[test]
    fn unknown_quantities_are_their_own_dimension() {
        let c = UnitCatalogue::new([CatalogueUnit {
            symbol: "lm".into(),
            quantity: "luminous flux".into(),
            to_base: 1.0,
            name: "lumen".into(),
        }]);
        assert_eq!(c.conversion_factor("lm", "s"), None);
        assert!(approx(c.conversion_factor("lm", "lm"), 1.0));
    }
}
