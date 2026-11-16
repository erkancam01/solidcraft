//! "Çift Açılım Şablonu": a parametric tilt-and-turn sash with its hardware, ported from the
//! hand-built test design (`tests/veri/cift_acilim_referans.json`, every command of which this
//! entry sends unchanged, in order), plus an extra lock and a child lock that configuration
//! rules switch on and off:
//!
//! - `EK KILIT 1` is suppressed while `FFH <= 1200 mm`,
//! - `COCUK KILIDI` is suppressed while `CK = 0`.
//!
//! The rules are SolidCraft's own (`config.rule`, `drive_suppression`): an ordinary edit of FFH
//! or CK in the Parameters panel runs them, so the parts come and go without the plugin.

use serde_json::{Value, json};

use crate::json::{mm, sayi, secim_bool, secim_sayi};
use crate::{Ana, Hata};

pub const EK_KILIT: &str = "EK KILIT 1";
pub const COCUK_KILIDI: &str = "COCUK KILIDI";

/// The parameters the template makes, in order; it refuses a design that has any of them.
pub const PARAMETRELER: &[&str] =
    &["FFH", "FFB", "KOL", "KA", "IS", "AM", "MKR", "PAY", "MK", "TIJ_SOL_ALT", "TIJ_SOL_UST", "TIJ_UST", "TIJ_ALT", "CK"];

const MK: &str = "220 mm + 200 mm * min(max(ceil((FFB - 650 mm) / (1 mm)), 0), 1) + 230 mm * min(max(ceil((FFB - 1000 mm) / (1 mm)), 0), 1)";

const GRAY: [u8; 3] = [205, 211, 218];
const DARK: [u8; 3] = [79, 86, 99];
const SILV: [u8; 3] = [175, 182, 194];
const PURP: [u8; 3] = [109, 76, 142];
const RED: [u8; 3] = [214, 39, 40];
const WHITE: [u8; 3] = [240, 240, 240];
const GLASS: [u8; 3] = [159, 200, 232];
const ORANGE: [u8; 3] = [230, 126, 34];
const GREEN: [u8; 3] = [46, 204, 113];

/// The user's inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Girdi {
    pub ffh: f64,
    pub ffb: f64,
    pub kol: f64,
    pub cocuk_kilidi: bool,
}

/// The system constants (mm), as the template's parameters set them.
const KA: f64 = 120.0;
const IS: f64 = 300.0;
const AM: f64 = 150.0;
const PAY: f64 = 1.0;

/// The scissor's top leg for a sash width: 220 up to 650, 420 up to 1000, 650 above.
pub fn makas(ffb: f64) -> f64 {
    let step = |over: f64| (over.ceil()).clamp(0.0, 1.0);
    220.0 + 200.0 * step(ffb - 650.0) + 230.0 * step(ffb - 1000.0)
}

/// The tie-bar cut lengths the parameters come to, by name.
pub fn tijler(g: &Girdi) -> [(&'static str, f64); 4] {
    [
        ("TIJ_SOL_ALT", g.kol - IS / 2.0 - KA - 2.0 * PAY),
        ("TIJ_SOL_UST", g.ffh - KA - g.kol - IS / 2.0 - 2.0 * PAY),
        ("TIJ_UST", g.ffb - KA - makas(g.ffb) - 2.0 * PAY),
        ("TIJ_ALT", g.ffb - KA - AM - 2.0 * PAY),
    ]
}

pub fn girdi(p: &Value) -> Result<Girdi, Hata> {
    let ffh = secim_sayi(p, "FFH", 1200.0, 600.0, 3000.0)?;
    let ffb = secim_sayi(p, "FFB", 900.0, 400.0, 2000.0)?;
    let kol = secim_sayi(p, "KOL", 600.0, 1.0, 3000.0)?;
    let cocuk_kilidi = secim_bool(p, "cocuk_kilidi", false)?;
    let g = Girdi { ffh, ffb, kol, cocuk_kilidi };
    for (ad, boy) in tijler(&g) {
        if boy <= 0.0 {
            let neden = match ad {
                "TIJ_SOL_ALT" => format!("Kol yüksekliği çok küçük: KOL en az {} mm olmalı.", sayi(IS / 2.0 + KA + 2.0 * PAY + 1.0)),
                "TIJ_SOL_UST" => {
                    format!("Kol yüksekliği çok büyük: KOL en çok {} mm olabilir (FFH - 273).", sayi(g.ffh - KA - IS / 2.0 - 2.0 * PAY - 1.0))
                }
                _ => "Kanat genişliği (FFB) çok küçük.".to_string(),
            };
            let alan = if ad.starts_with("TIJ_SOL") { "KOL" } else { "FFB" };
            return Err(Hata::girdi(alan, format!("{neden} ({ad} = {} mm olurdu.)", sayi(boy))));
        }
    }
    Ok(g)
}

fn parametre(name: &str, expression: &str) -> Value {
    json!({"command": "parameters.add", "params": {"name": name, "expression": expression}})
}

fn kutu(out: &mut Vec<Value>, name: &str, size: [&str; 3], center: [&str; 3], color: [u8; 3], opacity: Option<f64>) {
    out.push(json!({"command": "solid.box", "params": {
        "length": size[0], "width": size[1], "height": size[2], "center": center,
        "name": name, "body_name": name, "operation": "new"}}));
    let mut look = json!({"bodies": [name], "color": color});
    if let (Some(o), Some(m)) = (opacity, look.as_object_mut()) {
        m.insert("opacity".into(), json!(o));
    }
    out.push(json!({"command": "appearance.assign", "params": look}));
}

/// The geometry commands, in the reference design's order, then the two locks.
pub fn govde_komutlari(g: &Girdi) -> Vec<Value> {
    let mut c = vec![
        parametre("FFH", &mm(g.ffh)),
        parametre("FFB", &mm(g.ffb)),
        parametre("KOL", &mm(g.kol)),
        parametre("KA", "120 mm"),
        parametre("IS", "300 mm"),
        parametre("AM", "150 mm"),
        parametre("MKR", "180 mm"),
        parametre("PAY", "1 mm"),
        parametre("MK", MK),
        parametre("TIJ_SOL_ALT", "KOL - IS/2 - KA - 2*PAY"),
        parametre("TIJ_SOL_UST", "FFH - KA - KOL - IS/2 - 2*PAY"),
        parametre("TIJ_UST", "FFB - KA - MK - 2*PAY"),
        parametre("TIJ_ALT", "FFB - KA - AM - 2*PAY"),
    ];
    // Sash profile (62 x 70) and glass.
    kutu(&mut c, "Kanat sol", ["62 mm", "FFH", "70 mm"], ["31 mm", "FFH/2", "-35 mm"], GRAY, None);
    kutu(&mut c, "Kanat sag", ["62 mm", "FFH", "70 mm"], ["FFB - 31 mm", "FFH/2", "-35 mm"], GRAY, None);
    kutu(&mut c, "Kanat ust", ["FFB - 124 mm", "62 mm", "70 mm"], ["FFB/2", "FFH - 31 mm", "-35 mm"], GRAY, None);
    kutu(&mut c, "Kanat alt", ["FFB - 124 mm", "62 mm", "70 mm"], ["FFB/2", "31 mm", "-35 mm"], GRAY, None);
    kutu(&mut c, "Cam", ["FFB - 124 mm", "FFH - 124 mm", "6 mm"], ["FFB/2", "FFH/2", "-35 mm"], GLASS, Some(0.3));
    // Corner drives on the sash's outer edge (15 x 22 section).
    kutu(&mut c, "KA-02 dikey", ["15 mm", "KA + 15 mm", "22 mm"], ["-7.5 mm", "KA/2 - 7.5 mm", "-35 mm"], DARK, None);
    kutu(&mut c, "KA-02 yatay", ["KA", "15 mm", "22 mm"], ["KA/2", "-7.5 mm", "-35 mm"], DARK, None);
    kutu(&mut c, "KA-01 dikey", ["15 mm", "KA + 15 mm", "22 mm"], ["-7.5 mm", "FFH - KA/2 + 7.5 mm", "-35 mm"], DARK, None);
    kutu(&mut c, "KA-01 yatay", ["KA", "15 mm", "22 mm"], ["KA/2", "FFH + 7.5 mm", "-35 mm"], DARK, None);
    // Espagnolette and handle.
    kutu(&mut c, "IS-300 ispanyolet", ["15 mm", "IS", "22 mm"], ["-7.5 mm", "KOL", "-35 mm"], SILV, None);
    kutu(&mut c, "KOL rozet", ["32 mm", "76 mm", "14 mm"], ["31 mm", "KOL", "7 mm"], WHITE, None);
    kutu(&mut c, "KOL tutamak", ["20 mm", "150 mm", "16 mm"], ["31 mm", "KOL - 62 mm", "22 mm"], WHITE, None);
    // Scissor (its top leg follows FFB) and bottom hinge.
    kutu(&mut c, "Makas ust", ["MK + 15 mm", "15 mm", "22 mm"], ["FFB - MK/2 + 7.5 mm", "FFH + 7.5 mm", "-35 mm"], PURP, None);
    kutu(&mut c, "Makas sag", ["15 mm", "MKR", "22 mm"], ["FFB + 7.5 mm", "FFH - MKR/2", "-35 mm"], PURP, None);
    kutu(&mut c, "AM alt", ["AM + 15 mm", "15 mm", "22 mm"], ["FFB - AM/2 + 7.5 mm", "-7.5 mm", "-35 mm"], PURP, None);
    kutu(&mut c, "AM sag", ["15 mm", "AM", "22 mm"], ["FFB + 7.5 mm", "AM/2", "-35 mm"], PURP, None);
    // Tie bars, their lengths from the parameters.
    kutu(&mut c, "TIJ sol alt", ["7 mm", "TIJ_SOL_ALT", "10 mm"], ["-5 mm", "(KA + KOL - IS/2)/2", "-35 mm"], RED, None);
    kutu(&mut c, "TIJ sol ust", ["7 mm", "TIJ_SOL_UST", "10 mm"], ["-5 mm", "(KOL + IS/2 + FFH - KA)/2", "-35 mm"], RED, None);
    kutu(&mut c, "TIJ ust", ["TIJ_UST", "7 mm", "10 mm"], ["(KA + FFB - MK)/2", "FFH + 5 mm", "-35 mm"], RED, None);
    kutu(&mut c, "TIJ alt", ["TIJ_ALT", "7 mm", "10 mm"], ["(KA + FFB - AM)/2", "-5 mm", "-35 mm"], RED, None);
    // The extra lock, on the handle side halfway along the upper bar, and the child lock above
    // the handle rosette.
    kutu(&mut c, EK_KILIT, ["16 mm", "60 mm", "24 mm"], ["-7.5 mm", "(KOL + IS/2 + FFH - KA)/2", "-35 mm"], ORANGE, None);
    kutu(&mut c, COCUK_KILIDI, ["32 mm", "30 mm", "14 mm"], ["31 mm", "KOL + 58 mm", "7 mm"], GREEN, None);
    c.push(json!({"command": "parameters.add", "params": {
        "name": "CK", "expression": if g.cocuk_kilidi { "1" } else { "0" }, "unit": "", "comment": "Çocuk kilidi: 1 var, 0 yok"}}));
    c
}

/// The configuration rules for the two locks, given their feature ids.
pub fn kurallar(ek_kilit: u64, cocuk_kilidi: u64) -> Value {
    let rule = |id: &str, name: &str, parameter: &str, operator: &str, expression: &str, feature: u64, value: bool| {
        json!({"id": id, "name": name, "enabled": true,
            "condition": {"type": "parameter", "parameter": parameter, "operator": operator, "expression": expression},
            "effect": {"type": "drive_suppression", "feature": feature, "value": value}})
    };
    json!({"rules": [
        rule("ek-kilit-kapali", "Ek kilit: FFH 1200 mm ve altında yok", "FFH", "le", "1200 mm", ek_kilit, true),
        rule("ek-kilit-acik", "Ek kilit: FFH 1200 mm üstünde var", "FFH", "gt", "1200 mm", ek_kilit, false),
        rule("cocuk-kilidi-kapali", "Çocuk kilidi: CK = 0 ise yok", "CK", "eq", "0", cocuk_kilidi, true),
        rule("cocuk-kilidi-acik", "Çocuk kilidi: CK = 1 ise var", "CK", "ne", "0", cocuk_kilidi, false),
    ]})
}

fn feature_of(reply: &Value, name: &str) -> Result<u64, Hata> {
    reply.get("feature").and_then(Value::as_u64).ok_or_else(|| Hata::komut("solid.box", format!("`{name}` kutusunun özellik numarası okunamadı.")))
}

pub fn calistir(ana: &mut dyn Ana, p: &Value) -> Result<Value, Hata> {
    let g = girdi(p)?;
    let list = ana.komut("parameters.list", &json!({}))?;
    let var: Vec<&str> = list
        .get("parameters")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|p| p.get("name").and_then(Value::as_str)).collect())
        .unwrap_or_default();
    if let Some(ad) = PARAMETRELER.iter().find(|ad| var.contains(ad)) {
        return Err(Hata::girdi(
            "tasarım",
            format!("Bu tasarımda zaten `{ad}` parametresi var. Şablonu yeni (boş) bir tasarımda çalıştırın: Dosya > Yeni."),
        ));
    }
    let (mut ek, mut cocuk) = (None, None);
    for c in govde_komutlari(&g) {
        let name = c.get("command").and_then(Value::as_str).unwrap_or("");
        let params = c.get("params").cloned().unwrap_or(Value::Null);
        let reply = ana.komut(name, &params)?;
        if name == "solid.box" {
            match params.get("name").and_then(Value::as_str) {
                Some(EK_KILIT) => ek = Some(feature_of(&reply, EK_KILIT)?),
                Some(COCUK_KILIDI) => cocuk = Some(feature_of(&reply, COCUK_KILIDI)?),
                _ => {}
            }
        }
    }
    let (Some(ek), Some(cocuk)) = (ek, cocuk) else { return Err(Hata::komut("solid.box", "Kilit kutuları oluşmadı.")) };
    let table = ana.komut("config.configure", &json!({"params": ["FFH", "FFB", "KOL", "CK"], "features": [EK_KILIT, COCUK_KILIDI]}))?;
    let row = table
        .get("active")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| table.pointer("/rows/0/name").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| "Default".to_string());
    ana.komut("config.rule", &kurallar(ek, cocuk))?;
    // Rules switch suppression on edits and activations; activate once so they apply now.
    ana.komut("config.activate", &json!({"row": row}))?;

    let mut ozet = vec![
        format!("Çift açılım kanadı: FFH {} x FFB {} mm, kol yüksekliği {} mm.", sayi(g.ffh), sayi(g.ffb), sayi(g.kol)),
        format!("Makas boyu (MK): {} mm.", sayi(makas(g.ffb))),
    ];
    for (ad, boy) in tijler(&g) {
        ozet.push(format!("{ad}: {} mm", sayi(boy)));
    }
    ozet.push(format!("Ek kilit: {}", if g.ffh > 1200.0 { "var (FFH > 1200)" } else { "yok (FFH ≤ 1200)" }));
    ozet.push(format!("Çocuk kilidi: {}", if g.cocuk_kilidi { "var (CK = 1)" } else { "yok (CK = 0)" }));
    ozet.push("Ölçüleri Global Değişkenler panelinden değiştirin; kilitler kurallarla kendiliğinden açılıp kapanır.".into());
    ana.gunluk(&format!("çift açılım şablonu: FFH {} FFB {} KOL {}", sayi(g.ffh), sayi(g.ffb), sayi(g.kol)));
    Ok(json!({"ozet": ozet, "ek_kilit": ek, "cocuk_kilidi": cocuk, "satir": row}))
}

#[cfg(test)]
mod tests;
