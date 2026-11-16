//! Small, total helpers for reading host replies and entry parameters.

use serde_json::Value;

use crate::Hata;
use crate::vektor::V3;

/// `text` cut to at most `n` characters (for echoing untrusted names in messages).
pub fn kisalt(text: &str, n: usize) -> String {
    let mut out: String = text.chars().filter(|c| !c.is_control()).take(n).collect();
    if text.chars().count() > n {
        out.push('…');
    }
    out
}

/// A three-number array as a vector.
pub fn v3(value: Option<&Value>) -> Option<V3> {
    let a = value?.as_array()?;
    if a.len() != 3 {
        return None;
    }
    let x = a.first()?.as_f64()?;
    let y = a.get(1)?.as_f64()?;
    let z = a.get(2)?.as_f64()?;
    let v = V3::new(x, y, z);
    v.sonlu().then_some(v)
}

/// An optional boolean parameter; anything but a boolean is an input error.
pub fn secim_bool(p: &Value, alan: &str, varsayilan: bool) -> Result<bool, Hata> {
    match p.get(alan) {
        None | Some(Value::Null) => Ok(varsayilan),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(Hata::girdi(alan, "Evet/hayır (true/false) olmalı.")),
    }
}

/// An optional number parameter within `[min, max]`.
pub fn secim_sayi(p: &Value, alan: &str, varsayilan: f64, min: f64, max: f64) -> Result<f64, Hata> {
    let n = match p.get(alan) {
        None | Some(Value::Null) => varsayilan,
        Some(v) => v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| Hata::girdi(alan, "Bir sayı olmalı (mm)."))?,
    };
    if !(min..=max).contains(&n) {
        return Err(Hata::girdi(alan, format!("{} ile {} mm arasında olmalı; girilen: {}.", sayi(min), sayi(max), sayi(n))));
    }
    Ok(n)
}

/// An optional text parameter.
pub fn secim_metin<'a>(p: &'a Value, alan: &str) -> Result<Option<&'a str>, Hata> {
    match p.get(alan) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(Hata::girdi(alan, "Metin olmalı.")),
    }
}

/// A number as short text with a dot: `1200`, `1234.5`, `0.125`.
pub fn sayi(n: f64) -> String {
    let r = (n * 1e6).round() / 1e6;
    let r = if r == 0.0 { 0.0 } else { r };
    let mut s = format!("{r:.6}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

/// A number with a decimal comma, as Turkish Excel reads it: `1234,5`.
pub fn sayi_virgul(n: f64) -> String {
    sayi(n).replace('.', ",")
}

/// A length in millimetres as an expression: `1200 mm`.
pub fn mm(n: f64) -> String {
    format!("{} mm", sayi(n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_print_short() {
        assert_eq!(sayi(1200.0), "1200");
        assert_eq!(sayi(1234.5), "1234.5");
        assert_eq!(sayi(-0.0), "0");
        assert_eq!(sayi(0.1 + 0.2), "0.3");
        assert_eq!(sayi_virgul(327.5), "327,5");
        assert_eq!(mm(900.0), "900 mm");
    }

    #[test]
    fn hostile_values_are_input_errors() {
        let p = json!({"a": "x", "b": [1, 2], "c": "1e400", "d": true});
        assert!(secim_bool(&p, "a", false).is_err());
        assert!(secim_sayi(&p, "b", 1.0, 0.0, 2.0).is_err());
        assert!(secim_sayi(&p, "c", 1.0, 0.0, 2.0).is_err());
        assert!(secim_metin(&p, "d").is_err());
        assert_eq!(secim_bool(&p, "yok", true), Ok(true));
        assert_eq!(v3(Some(&json!([1, 2]))), None);
        assert_eq!(v3(Some(&json!([1, "2", 3]))), None);
        assert_eq!(kisalt("abcdef", 3), "abc…");
    }
}
