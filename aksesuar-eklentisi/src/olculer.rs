//! "Ölçüleri Değiştir": changes the template's sizes (FFH, FFB, KOL) and the child lock (CK) in
//! one parameter edit, so SolidCraft's suppression rules run on it: the template's locks and
//! bars, and the parts "Koşullu Parçaları Güncelle" made conditional, come and go with it.
//!
//! Every field is optional: one left empty keeps its value. The new sizes are checked with the
//! template's own limits (tie bars long enough) before anything changes.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::json::{kisalt, mm, sayi};
use crate::kosullu::{self, Nokta};
use crate::{Ana, Hata};

/// The sizes this entry changes, with their limits (mm), as the template's dialog has them.
pub const OLCULER: &[(&str, f64, f64)] = &[("FFH", 600.0, 3000.0), ("FFB", 400.0, 2000.0), ("KOL", 273.0, 2727.0)];
/// The child lock choice: keep, yes (CK = 1), no (CK = 0).
pub const DEGISTIRME: &str = "degistirme";
pub const EVET: &str = "evet";
pub const HAYIR: &str = "hayir";

const SABLON_YOK: &str = "Önce Çift Açılım Şablonu'nu çalıştırın (Dosya > Yeni, sonra Çift Açılım Şablonu).";

/// What the user asked for: the sizes to set (mm) and the child lock.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Istek {
    pub olculer: Vec<(&'static str, f64)>,
    pub cocuk_kilidi: Option<bool>,
}

/// An optional size: absent or null keeps it; else a number within its limits.
fn olcu(p: &Value, alan: &str, min: f64, max: f64) -> Result<Option<f64>, Hata> {
    match p.get(alan) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => crate::json::secim_sayi(p, alan, min, min, max).map(Some),
    }
}

pub fn istek(p: &Value) -> Result<Istek, Hata> {
    let mut olculer = Vec::new();
    for (ad, min, max) in OLCULER {
        if let Some(n) = olcu(p, ad, *min, *max)? {
            olculer.push((*ad, n));
        }
    }
    let cocuk_kilidi = match p.get("cocuk_kilidi") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(b)) => Some(*b),
        Some(Value::String(s)) => match s.trim() {
            "" | DEGISTIRME => None,
            EVET => Some(true),
            HAYIR => Some(false),
            _ => return Err(Hata::girdi("cocuk_kilidi", "Değiştirme, Evet ya da Hayır olmalı.")),
        },
        Some(_) => return Err(Hata::girdi("cocuk_kilidi", "Değiştirme, Evet ya da Hayır olmalı.")),
    };
    if olculer.is_empty() && cocuk_kilidi.is_none() {
        return Err(Hata::girdi("olculer", "Hiçbir değer girilmedi: değiştirmek istediğiniz ölçüyü (FFH, FFB, KOL) ya da çocuk kilidini seçin."));
    }
    Ok(Istek { olculer, cocuk_kilidi })
}

/// A parameter's value now (mm for lengths) and its expression, by name, from `document.inspect`.
fn degerler(belge: &Value) -> BTreeMap<String, (Option<f64>, String)> {
    belge
        .get("params")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let ad = p.get("name")?.as_str()?.to_string();
            let deger = p.get("value").and_then(Value::as_f64).filter(|v| v.is_finite());
            Some((ad, (deger, p.get("expression").and_then(Value::as_str).unwrap_or("").to_string())))
        })
        .collect()
}

fn ayni(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * a.abs().max(b.abs()).max(1.0)
}

/// The conditional slots of a `hardware.slot.list` reply, by id.
fn kosullular(reply: &Value) -> BTreeMap<String, Nokta> {
    kosullu::noktalar(reply).into_iter().filter(|n| n.kosul.is_some()).map(|n| (n.id.clone(), n)).collect()
}

fn kullanim(aktif: bool) -> &'static str {
    if aktif { "kullanımda" } else { "kullanımda değil" }
}

pub fn calistir(ana: &mut dyn Ana, p: &Value) -> Result<Value, Hata> {
    let istek = istek(p)?;
    let belge = ana.komut("document.inspect", &json!({"timeline": false, "bodies": false}))?;
    let simdi = degerler(&belge);
    let eksik: Vec<&str> = OLCULER.iter().map(|(ad, _, _)| *ad).filter(|ad| !simdi.contains_key(*ad)).collect();
    if !eksik.is_empty() {
        return Err(Hata::girdi("tasarım", format!("Bu tasarımda {} parametresi yok. {SABLON_YOK}", eksik.join(", "))));
    }
    if istek.cocuk_kilidi.is_some() && !simdi.contains_key("CK") {
        return Err(Hata::girdi("cocuk_kilidi", format!("Bu tasarımda CK (çocuk kilidi) parametresi yok. {SABLON_YOK}")));
    }
    let eski = |ad: &str| simdi.get(ad).and_then(|(v, _)| *v);
    // The sizes they come to, checked as the template checks them.
    let yeni = |ad: &str| istek.olculer.iter().find(|(n, _)| *n == ad).map(|(_, v)| *v).or_else(|| eski(ad));
    if let (Some(ffh), Some(ffb), Some(kol)) = (yeni("FFH"), yeni("FFB"), yeni("KOL")) {
        crate::sablon::girdi(&json!({"FFH": ffh, "FFB": ffb, "KOL": kol}))?;
    }
    let mut degisiklik = Map::new();
    let mut satirlar = Vec::new();
    for (ad, v) in &istek.olculer {
        let once = eski(ad);
        if once.is_some_and(|o| ayni(o, *v)) {
            continue;
        }
        degisiklik.insert((*ad).to_string(), json!(mm(*v)));
        satirlar.push(format!("{ad}: {} → {} mm", once.map_or_else(|| "?".to_string(), sayi), sayi(*v)));
    }
    if let Some(ck) = istek.cocuk_kilidi {
        let hedef = if ck { 1.0 } else { 0.0 };
        let once = eski("CK");
        if !once.is_some_and(|o| ayni(o, hedef)) {
            degisiklik.insert("CK".into(), json!(if ck { "1" } else { "0" }));
            satirlar.push(format!(
                "CK: {} → {} (çocuk kilidi {})",
                once.map_or_else(|| "?".to_string(), sayi),
                sayi(hedef),
                if ck { "var" } else { "yok" }
            ));
        }
    }
    if degisiklik.is_empty() {
        return Ok(json!({"ozet": ["Ölçüler zaten bu değerlerde; bir şey değişmedi."], "degisen": {}, "noktalar": []}));
    }

    let once = kosullular(&ana.komut("hardware.slot.list", &json!({}))?);
    let kurallar = kosullu::mevcut_kurallar(&ana.komut("config.rules", &json!({}))?);
    let cevap = ana.komut("parameters.change", &json!({"changes": degisiklik}))?;
    let sonra = kosullular(&ana.komut("hardware.slot.list", &json!({}))?);

    let mut ozet = vec!["Ölçüler değişti:".to_string()];
    ozet.extend(satirlar.iter().map(|s| format!("• {s}")));
    // What the rules switched, by name.
    let mut gelen = Vec::new();
    let mut giden = Vec::new();
    for s in cevap.get("switched").and_then(Value::as_array).into_iter().flatten() {
        let ad = s.get("name").and_then(Value::as_str).map(str::to_string).or_else(|| s.get("feature").map(Value::to_string)).unwrap_or_default();
        if s.get("suppressed").and_then(Value::as_bool).unwrap_or(false) { giden.push(ad) } else { gelen.push(ad) }
    }
    if !gelen.is_empty() {
        ozet.push(format!("Kurallarla gelen: {}", kisalt(&gelen.join(", "), 400)));
    }
    if !giden.is_empty() {
        ozet.push(format!("Kurallarla bastırılan: {}", kisalt(&giden.join(", "), 400)));
    }
    let mut noktalar = Vec::new();
    let mut nokta_satirlari = Vec::new();
    for (id, n) in &sonra {
        let onceki = once.get(id).map(|o| o.aktif);
        if onceki == Some(n.aktif) {
            continue;
        }
        let kuralli = n.parca.is_some() && kurallar.iter().any(|(r, _)| kosullu::noktanin_kurali(r, &kosullu::kural_koku(id)));
        let mut satir = format!("• {}: {} → {}", kisalt(&n.etiket, 60), onceki.map_or("?", kullanim), kullanim(n.aktif));
        if n.parca.is_some() {
            if kuralli {
                satir.push_str(if n.aktif { " (parça geldi)" } else { " (parça bastırıldı)" });
            } else {
                satir.push_str(" — parçası koşullu değil: 'Koşullu Parçaları Güncelle'yi çalıştırın");
            }
        }
        nokta_satirlari.push(satir);
        noktalar.push(json!({"nokta": id, "once": onceki, "sonra": n.aktif, "parca": n.parca.is_some(), "kuralli": kuralli}));
    }
    if !nokta_satirlari.is_empty() {
        ozet.push("Koşullu yerleşim noktaları:".into());
        ozet.extend(nokta_satirlari);
    }
    if let Some(hatalar) = cevap.get("errors").and_then(Value::as_array).filter(|h| !h.is_empty()) {
        let metin: Vec<String> = hatalar.iter().map(|h| h.as_str().map(str::to_string).unwrap_or_else(|| h.to_string())).collect();
        ozet.push(format!("Uyarı: yeniden hesaplamada hata var: {}", kisalt(&metin.join("; "), 300)));
    }
    ana.gunluk(&format!("ölçüler: {}", satirlar.join(", ")));
    Ok(json!({"ozet": ozet, "degisen": degisiklik, "noktalar": noktalar, "gelen": gelen, "bastirilan": giden}))
}

#[cfg(test)]
mod tests;
