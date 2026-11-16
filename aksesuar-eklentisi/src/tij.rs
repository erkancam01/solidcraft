//! "Tij ve Malzeme Listesi": a cut list of the tie bars (TIJ…) and a parts count, written as a
//! CSV that Turkish Excel opens directly (UTF-8 with BOM, `;` between fields, decimal comma).

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::json::{kisalt, sayi, sayi_virgul, secim_metin};
use crate::{Ana, Hata};

/// The folder grant the CSV is written to (named in plugin.json).
pub const IZIN: &str = "cikti";
pub const VARSAYILAN_DOSYA: &str = "tij_listesi.csv";

/// One body of the design, as `document.inspect` measures it.
#[derive(Clone, Debug, PartialEq)]
pub struct Govde {
    pub ad: String,
    /// The axis-aligned box's sizes (mm); `None` when it could not be measured.
    pub olcu: Option<[f64; 3]>,
}

/// A user parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct Parametre {
    pub ad: String,
    pub ifade: String,
    pub deger: Option<f64>,
    pub birim: String,
}

/// Whether `ad` names a tie bar: it starts with "TIJ", in any case (Turkish `İ`/`ı` too).
pub fn tij_mi(ad: &str) -> bool {
    let head: String = ad
        .trim_start()
        .chars()
        .take(3)
        .map(|c| match c {
            'İ' | 'ı' | 'i' => 'I',
            c => c.to_ascii_uppercase(),
        })
        .collect();
    head == "TIJ"
}

/// A bar's cut length: its longest box side, to the nearest 0.5 mm.
pub fn kesim_boyu(olcu: [f64; 3]) -> f64 {
    let longest = olcu.into_iter().fold(0.0_f64, f64::max);
    (longest * 2.0).round() / 2.0
}

/// A part's code: its name up to the first space.
pub fn kod(ad: &str) -> String {
    let t = ad.trim();
    t.split_whitespace().next().unwrap_or(t).to_string()
}

/// A CSV field: quoted when it holds `;`, a quote or a line break, and with a leading `'` when
/// Excel would read it as a formula.
pub fn alan(text: &str) -> String {
    let mut t: String = text.chars().map(|c| if c == '\r' || c == '\n' { ' ' } else { c }).collect();
    if t.starts_with(['=', '+', '-', '@']) {
        t.insert(0, '\'');
    }
    if t.contains([';', '"']) { format!("\"{}\"", t.replace('"', "\"\"")) } else { t }
}

/// The bodies of a `document.inspect {measure: true}` reply.
pub fn govdeler(reply: &Value) -> Vec<Govde> {
    let Some(list) = reply.get("bodies").and_then(Value::as_array) else { return Vec::new() };
    list.iter()
        .filter_map(|b| {
            let ad = b.get("name")?.as_str()?.to_string();
            let olcu = b.get("bbox").and_then(|bb| {
                let min = crate::json::v3(bb.get("min"))?;
                let max = crate::json::v3(bb.get("max"))?;
                Some([(max.x - min.x).abs(), (max.y - min.y).abs(), (max.z - min.z).abs()])
            });
            Some(Govde { ad, olcu })
        })
        .collect()
}

/// The user parameters of a `document.inspect` reply (not those dimensions and features made).
pub fn parametreler(reply: &Value) -> Vec<Parametre> {
    let Some(list) = reply.get("params").and_then(Value::as_array) else { return Vec::new() };
    list.iter()
        .filter(|p| p.get("model").and_then(Value::as_bool) != Some(true))
        .filter_map(|p| {
            Some(Parametre {
                ad: p.get("name")?.as_str()?.to_string(),
                ifade: p.get("expression").and_then(Value::as_str).unwrap_or("").to_string(),
                deger: p.get("value").and_then(Value::as_f64).filter(|v| v.is_finite()),
                birim: p.get("unit").and_then(Value::as_str).unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// The list, worked out once for both the CSV and the summary.
#[derive(Clone, Debug, PartialEq)]
pub struct Liste {
    /// Tie bars: name and cut length (`None`: not measured).
    pub tijler: Vec<(String, Option<f64>)>,
    /// Other parts by code: count and their names.
    pub parcalar: BTreeMap<String, Vec<String>>,
    pub parametreler: Vec<Parametre>,
}

pub fn liste(govdeler: &[Govde], parametreler: Vec<Parametre>) -> Liste {
    let mut tijler = Vec::new();
    let mut parcalar: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for g in govdeler {
        if tij_mi(&g.ad) {
            tijler.push((g.ad.clone(), g.olcu.map(kesim_boyu)));
        } else {
            parcalar.entry(kod(&g.ad)).or_default().push(g.ad.clone());
        }
    }
    tijler.sort_by(|a, b| a.0.cmp(&b.0));
    Liste { tijler, parcalar, parametreler }
}

impl Liste {
    pub fn toplam_tij(&self) -> f64 {
        self.tijler.iter().filter_map(|(_, l)| *l).sum()
    }

    /// The CSV bytes: BOM, three sections, CRLF lines.
    pub fn csv(&self, tasarim: &str) -> Vec<u8> {
        let mut rows: Vec<String> = Vec::new();
        rows.push(format!("TİJ VE MALZEME LİSTESİ;{}", alan(tasarim)));
        rows.push(String::new());
        rows.push("TİJLER".into());
        rows.push("Ad;Kesim boyu (mm)".into());
        for (ad, boy) in &self.tijler {
            rows.push(format!("{};{}", alan(ad), boy.map(sayi_virgul).unwrap_or_else(|| "ölçülemedi".into())));
        }
        rows.push(format!("Toplam;{}", sayi_virgul(self.toplam_tij())));
        rows.push(String::new());
        rows.push("PARÇALAR".into());
        rows.push("Kod;Adet;Gövdeler".into());
        for (kod, adlar) in &self.parcalar {
            rows.push(format!("{};{};{}", alan(kod), adlar.len(), alan(&adlar.join(", "))));
        }
        rows.push(String::new());
        rows.push("PARAMETRELER".into());
        rows.push("Ad;Değer;Birim;İfade".into());
        for p in &self.parametreler {
            let deger = p.deger.map(sayi_virgul).unwrap_or_default();
            rows.push(format!("{};{};{};{}", alan(&p.ad), deger, alan(&p.birim), alan(&p.ifade)));
        }
        let mut out = vec![0xEF, 0xBB, 0xBF];
        for row in rows {
            out.extend_from_slice(row.as_bytes());
            out.extend_from_slice(b"\r\n");
        }
        out
    }

    /// Lines for the entry's result dialog.
    pub fn ozet(&self, dosya: &str) -> Vec<String> {
        let mut out = vec![format!("{} tij, toplam {} mm:", self.tijler.len(), sayi(self.toplam_tij()))];
        for (ad, boy) in &self.tijler {
            out.push(format!("  {ad}: {}", boy.map_or_else(|| "ölçülemedi".to_string(), |b| format!("{} mm", sayi(b)))));
        }
        let adet: usize = self.parcalar.values().map(Vec::len).sum();
        out.push(format!("{} parça, {} kod:", adet, self.parcalar.len()));
        for (kod, adlar) in &self.parcalar {
            out.push(format!("  {kod}: {} adet", adlar.len()));
        }
        out.push(format!("Dosya: {IZIN}/{dosya}"));
        out
    }
}

/// The file name the user gave, made safe: a plain name in the granted folder ending in `.csv`.
pub fn dosya_adi(p: &Value) -> Result<String, Hata> {
    let ad = secim_metin(p, "dosya")?.map(str::trim).filter(|s| !s.is_empty()).unwrap_or(VARSAYILAN_DOSYA);
    if ad.chars().count() > 100 {
        return Err(Hata::girdi("dosya", "Dosya adı en çok 100 karakter olabilir."));
    }
    if ad.starts_with('.') || ad.contains("..") {
        return Err(Hata::girdi("dosya", "Dosya adı nokta ile başlayamaz ve `..` içeremez."));
    }
    if let Some(c) = ad.chars().find(|c| !(c.is_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.' | '(' | ')'))) {
        return Err(Hata::girdi("dosya", format!("Dosya adında izin verilmeyen karakter: `{c}` (klasör yazılamaz; yalnızca dosya adı).")));
    }
    Ok(if ad.to_lowercase().ends_with(".csv") { ad.to_string() } else { format!("{ad}.csv") })
}

pub fn calistir(ana: &mut dyn Ana, p: &Value) -> Result<Value, Hata> {
    let dosya = dosya_adi(p)?;
    let reply = ana.komut("document.inspect", &json!({"measure": true, "timeline": false}))?;
    let tasarim = reply.get("name").and_then(Value::as_str).unwrap_or("").to_string();
    let govdeler = govdeler(&reply);
    if govdeler.is_empty() {
        return Err(Hata::girdi("tasarım", "Tasarımda gövde yok; listelenecek bir şey bulunamadı."));
    }
    let liste = liste(&govdeler, parametreler(&reply));
    ana.dosya_yaz(IZIN, &dosya, &liste.csv(&tasarim))?;
    let ozet = liste.ozet(&dosya);
    ana.gunluk(&format!("tij listesi: {} tij, {} kod -> {}", liste.tijler.len(), liste.parcalar.len(), kisalt(&dosya, 100)));
    Ok(json!({"ozet": ozet, "dosya": format!("{IZIN}/{dosya}"), "tij_sayisi": liste.tijler.len(), "toplam_tij_mm": liste.toplam_tij()}))
}

#[cfg(test)]
mod tests;
