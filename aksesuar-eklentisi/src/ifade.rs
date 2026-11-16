//! Distance expressions typed by the user (`KOL`, `FFH/2`, `250`, `250,5`): checked before they
//! reach SolidCraft, so a typo gets a Turkish message naming it.

use serde_json::{Value, json};

use crate::Hata;

/// Longest expression accepted.
pub const EN_UZUN: usize = 200;

/// A distance: a plain number of millimetres, or an expression SolidCraft keeps and re-evaluates.
#[derive(Clone, Debug, PartialEq)]
pub enum Mesafe {
    Sayi(f64),
    Ifade(String),
}

impl Mesafe {
    /// As a joint value: a number, or the expression text.
    pub fn json(&self) -> Value {
        match self {
            Mesafe::Sayi(n) => json!(n),
            Mesafe::Ifade(s) => json!(s),
        }
    }
    /// The parameter names the expression reads (empty for a number).
    pub fn adlar(&self) -> Vec<String> {
        match self {
            Mesafe::Sayi(_) => Vec::new(),
            Mesafe::Ifade(s) => tanimlayicilar(s),
        }
    }
}

/// Unit and function names SolidCraft expressions know, which are not parameters.
const BILINEN: &[&str] = &[
    "abs", "acos", "and", "asin", "atan", "atan2", "ceil", "cm", "cos", "deg", "e", "exp", "false", "floor", "ft", "if", "in", "inch", "ln", "log",
    "log10", "m", "max", "min", "mm", "not", "or", "pi", "pow", "rad", "round", "sign", "sin", "sqrt", "tan", "true", "um",
];

/// Reads `metin` as a distance. `alan` names the field in errors.
pub fn mesafe(alan: &str, metin: &str) -> Result<Mesafe, Hata> {
    let t = metin.trim();
    if t.is_empty() {
        return Err(Hata::girdi(alan, "Mesafe boş olamaz. Bir sayı (mm) ya da KOL, FFH/2 gibi bir ifade yazın."));
    }
    if t.chars().count() > EN_UZUN {
        return Err(Hata::girdi(alan, format!("Mesafe en çok {EN_UZUN} karakter olabilir.")));
    }
    // A plain number, with a decimal dot or comma.
    if let Ok(n) = t.replace(',', ".").parse::<f64>()
        && t.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+'))
    {
        if !n.is_finite() || n.abs() > 1e6 {
            return Err(Hata::girdi(alan, "Mesafe çok büyük."));
        }
        return Ok(Mesafe::Sayi(n));
    }
    if let Some(c) = t.chars().find(|c| !(c.is_alphanumeric() || matches!(c, ' ' | '+' | '-' | '*' | '/' | '(' | ')' | '.' | ',' | '_' | '^'))) {
        return Err(Hata::girdi(alan, format!("Mesafede izin verilmeyen karakter var: `{c}`.")));
    }
    let mut derinlik = 0i32;
    for c in t.chars() {
        match c {
            '(' => derinlik += 1,
            ')' => {
                derinlik -= 1;
                if derinlik < 0 {
                    return Err(Hata::girdi(alan, "Parantezler eşleşmiyor: fazladan `)` var."));
                }
            }
            _ => {}
        }
    }
    if derinlik != 0 {
        return Err(Hata::girdi(alan, "Parantezler eşleşmiyor: kapanmayan `(` var."));
    }
    if t.ends_with(['+', '-', '*', '/', '^']) {
        return Err(Hata::girdi(alan, "İfade bir işaretle bitemez."));
    }
    Ok(Mesafe::Ifade(t.to_string()))
}

/// The identifiers of an expression that are not units or functions, in order, once each.
pub fn tanimlayicilar(ifade: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        let starts_with_letter = cur.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_');
        if starts_with_letter && !BILINEN.contains(&cur.to_lowercase().as_str()) && !out.contains(cur) {
            out.push(cur.clone());
        }
        cur.clear();
    };
    for c in ifade.chars() {
        if c.is_alphanumeric() || c == '_' || (c == '.' && cur.chars().next().is_some_and(|f| f.is_ascii_digit())) {
            cur.push(c);
        } else {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

/// Fails when the expression reads a name that is not among `parametreler`.
pub fn adlar_var_mi(alan: &str, m: &Mesafe, parametreler: &[String]) -> Result<(), Hata> {
    for ad in m.adlar() {
        if !parametreler.iter().any(|p| p == &ad) {
            return Err(Hata::girdi(
                alan,
                format!(
                    "Tasarımda `{}` adlı parametre yok. Global Değişkenler panelindeki adı aynen yazın (büyük/küçük harf önemli).",
                    crate::json::kisalt(&ad, 40)
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_take_a_dot_or_a_comma() {
        assert_eq!(mesafe("m", "250"), Ok(Mesafe::Sayi(250.0)));
        assert_eq!(mesafe("m", " 250,5 "), Ok(Mesafe::Sayi(250.5)));
        assert_eq!(mesafe("m", "-12.25"), Ok(Mesafe::Sayi(-12.25)));
        assert_eq!(Mesafe::Sayi(250.5).json(), json!(250.5));
    }

    #[test]
    fn expressions_are_kept_as_written() {
        for e in ["KOL", "FFH/2", "FFH / 2 - 15 mm", "max(KOL, 300 mm)", "(FFB - KA)/2", "KOL_2*2"] {
            assert_eq!(mesafe("m", e), Ok(Mesafe::Ifade(e.to_string())), "{e}");
        }
        assert_eq!(Mesafe::Ifade("FFH/2".into()).json(), json!("FFH/2"));
    }

    #[test]
    fn bad_expressions_say_why_in_turkish() {
        let why = |e: &str| match mesafe("mesafe", e) {
            Err(Hata::Girdi { alan, neden }) => {
                assert_eq!(alan, "mesafe");
                neden
            }
            other => format!("{other:?}"),
        };
        assert!(why("").contains("boş"));
        assert!(why("   ").contains("boş"));
        assert!(why("KOL; rm").contains("`;`"));
        assert!(why("=KOL").contains("`=`"));
        assert!(why("(KOL").contains("kapanmayan"));
        assert!(why("KOL)").contains("fazladan"));
        assert!(why("KOL +").contains("işaret"));
        assert!(why(&"K".repeat(201)).contains("200"));
    }

    #[test]
    fn identifiers_leave_out_units_and_functions() {
        assert_eq!(tanimlayicilar("FFH/2 - 15 mm"), vec!["FFH"]);
        assert_eq!(tanimlayicilar("max(KOL, IS/2) + KOL + 2.5mm"), vec!["KOL", "IS"]);
        assert_eq!(tanimlayicilar("ceil((FFB - 650 mm) / (1 mm))"), vec!["FFB"]);
        assert!(tanimlayicilar("250").is_empty());
        let params = vec!["KOL".to_string(), "FFH".to_string()];
        assert!(adlar_var_mi("m", &Mesafe::Ifade("FFH/2".into()), &params).is_ok());
        let err = adlar_var_mi("m", &Mesafe::Ifade("KOLL + 1 mm".into()), &params);
        assert!(matches!(err, Err(Hata::Girdi { ref neden, .. }) if neden.contains("`KOLL`")), "{err:?}");
    }
}
