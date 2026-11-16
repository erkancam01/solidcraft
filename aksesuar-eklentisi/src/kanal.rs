//! "Kanala Yerleştir": seats a hardware part in a sash groove.
//!
//! The user selects the part's seat face and a straight edge of the sash (and, optionally, the
//! groove floor face beside that edge), then runs the entry with a distance. The entry makes one
//! planar joint, SolidCraft's face-to-face mate:
//!
//! - side a is the sash: an edge origin at the corner where the edge starts (or ends, `sondan`),
//!   on the groove floor. SolidCraft finds the edge, the floor and the corner again by name on
//!   every solve, so the part stays the same distance from that corner when the sash changes size.
//!   Its frame: z out of the floor; x and y along the floor's two edges at the corner, ordered
//!   right-handed (`joints_edge.rs` in the engine).
//! - side b is the part: a face origin at the centre of the seat face, z out of it, x along the
//!   part's longest straight edge in that face.
//!
//! The mate puts the seat face on the floor (b's z against a's). The joint's three values are the
//! shift along a's x and y and the turn about z: the shift along the edge is the user's distance
//! (kept as an expression, re-evaluated when parameters change); the shift across the groove (to
//! the floor's middle) and the turn (part along the edge, or turned 180 degrees with `cevir`) are
//! fixed by limits of zero width, so only the distance moves the part.

use serde_json::{Value, json};

use crate::ifade::{self, Mesafe};
use crate::json::{kisalt, secim_bool, secim_metin, v3};
use crate::vektor::V3;
use crate::{Ana, Hata};

/// A selected face or edge, as `selection.get` gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct Oge {
    pub govde: String,
    pub indeks: u64,
    /// Where it was picked (world), when the selection says.
    pub nokta: Option<V3>,
    /// The occurrence holding the body, as joint origins take it: an id, a component name, or 0
    /// for the design's own bodies.
    pub yerlesim: Value,
}

/// The straight edge of the sash.
#[derive(Clone, Debug, PartialEq)]
pub struct Kenar {
    pub oge: Oge,
    pub bas: V3,
    pub son: V3,
}

/// What the selection holds, sorted out.
#[derive(Clone, Debug, PartialEq)]
pub struct Secim {
    pub oturma: Oge,
    pub kenar: Kenar,
    pub taban: Option<Oge>,
}

const SECIM_YARDIM: &str = "Önce aksesuarın oturma yüzünü, sonra kanat profilinin düz kenarını seçin (Ctrl ile çoklu seçim). \
                            İsterseniz kanal tabanı yüzünü de seçebilirsiniz.";

fn oge(item: &Value) -> Option<Oge> {
    let govde = item.get("body")?.as_str()?.to_string();
    let indeks = item.get("index")?.as_u64()?;
    let nokta = v3(item.get("point"));
    let yerlesim = match item.get("occurrence_path").and_then(Value::as_array).and_then(|p| p.last()).and_then(Value::as_u64) {
        Some(id) => json!(id),
        None => match item.get("component") {
            Some(c) if c.get("id").and_then(Value::as_u64).is_some_and(|id| id != 0) => {
                c.get("name").and_then(Value::as_str).map_or_else(|| json!(0), |n| json!(n))
            }
            _ => json!(0),
        },
    };
    Some(Oge { govde, indeks, nokta, yerlesim })
}

/// Sorts the selection into seat face, sash edge and optional floor face.
pub fn secimi_oku(reply: &Value) -> Result<Secim, Hata> {
    let items = reply.get("items").and_then(Value::as_array).ok_or_else(|| Hata::komut("selection.get", "Seçim okunamadı."))?;
    if items.is_empty() {
        return Err(Hata::girdi("seçim", format!("Hiçbir şey seçili değil. {SECIM_YARDIM}")));
    }
    let kind = |i: &Value| i.get("kind").or_else(|| i.get("type")).and_then(Value::as_str).unwrap_or("").to_string();
    let edges: Vec<&Value> = items.iter().filter(|i| kind(i) == "edge").collect();
    let faces: Vec<&Value> = items.iter().filter(|i| kind(i) == "face").collect();
    if let Some(other) = items.iter().map(kind).find(|k| k != "edge" && k != "face") {
        return Err(Hata::girdi("seçim", format!("Seçimde yüz ve kenar dışında bir öğe var ({}). {SECIM_YARDIM}", kisalt(&other, 30))));
    }
    let [edge] = edges.as_slice() else {
        return Err(Hata::girdi("seçim", format!("Tam bir kenar seçin; seçili kenar sayısı: {}. {SECIM_YARDIM}", edges.len())));
    };
    if let Some(curve) = edge.get("curve").and_then(Value::as_str)
        && curve != "line"
    {
        return Err(Hata::girdi("seçim", "Seçilen kenar düz değil (yay ya da eğri). Kanat profilinin düz bir kenarını seçin."));
    }
    let kenar_oge = oge(edge).ok_or_else(|| Hata::girdi("seçim", "Seçilen kenarın gövdesi okunamadı."))?;
    let (Some(bas), Some(son)) = (v3(edge.get("start")), v3(edge.get("end"))) else {
        return Err(Hata::girdi("seçim", "Seçilen kenarın uçları okunamadı; düz bir model kenarı seçin (çizim çizgisi değil)."));
    };
    let mut oturma = None;
    let mut taban = None;
    for face in faces {
        let f = oge(face).ok_or_else(|| Hata::girdi("seçim", "Seçilen yüzün gövdesi okunamadı."))?;
        let slot = if f.govde == kenar_oge.govde { &mut taban } else { &mut oturma };
        if slot.is_some() {
            return Err(Hata::girdi("seçim", "Fazla yüz seçili. Aksesuarda bir yüz (oturma yüzü), kanatta en çok bir yüz (kanal tabanı) seçin."));
        }
        *slot = Some(f);
    }
    let oturma = oturma
        .ok_or_else(|| Hata::girdi("seçim", format!("Aksesuarın oturma yüzü seçili değil (kenardan başka bir gövdede bir yüz). {SECIM_YARDIM}")))?;
    Ok(Secim { oturma, kenar: Kenar { oge: kenar_oge, bas, son }, taban })
}

/// A planar face from `model.faces`: outward normal, centroid, a point that picks it, area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Yuz {
    pub normal: V3,
    pub merkez: V3,
    pub nokta: V3,
    pub alan: f64,
}

fn yuz_of(face: &Value) -> Option<Yuz> {
    let normal = v3(face.get("plane_normal"))?.birim()?;
    let merkez = v3(face.get("centroid"))?;
    let nokta = v3(face.get("point")).unwrap_or(merkez);
    let alan = face.get("area").and_then(Value::as_f64).filter(|a| a.is_finite()).unwrap_or(0.0);
    Some(Yuz { normal, merkez, nokta, alan })
}

/// Face `indeks` of a `model.faces` reply; `ne` names it in the error.
pub fn yuz(reply: &Value, indeks: u64, ne: &str) -> Result<Yuz, Hata> {
    let faces = reply.get("faces").and_then(Value::as_array).ok_or_else(|| Hata::komut("model.faces", "Yüzler okunamadı."))?;
    let face = faces
        .iter()
        .find(|f| f.get("index").and_then(Value::as_u64) == Some(indeks))
        .ok_or_else(|| Hata::girdi("seçim", format!("{ne} modelde bulunamadı; yeniden seçin.")))?;
    yuz_of(face).ok_or_else(|| Hata::girdi("seçim", format!("{ne} düz (düzlemsel) bir yüz olmalı.")))
}

/// Whether `p` lies in the plane of `y`, within `tol` mm.
fn duzlemde(y: &Yuz, p: V3, tol: f64) -> bool {
    y.normal.nokta(p.eksi(y.merkez)).abs() <= tol
}

fn tolerans(k: &Kenar) -> f64 {
    (k.son.eksi(k.bas).boy() * 1e-6).max(1e-3)
}

/// Whether the edge lies in face `y`'s plane.
pub fn kenari_tasir(y: &Yuz, k: &Kenar) -> bool {
    let tol = tolerans(k);
    duzlemde(y, k.bas, tol) && duzlemde(y, k.son, tol)
}

/// The groove floor when none was selected: the larger planar face whose plane holds the edge,
/// as SolidCraft's own edge origin picks it by default.
pub fn tabani_bul(reply: &Value, k: &Kenar) -> Result<Yuz, Hata> {
    let faces = reply.get("faces").and_then(Value::as_array).ok_or_else(|| Hata::komut("model.faces", "Yüzler okunamadı."))?;
    faces
        .iter()
        .filter_map(yuz_of)
        .filter(|y| kenari_tasir(y, k))
        .max_by(|a, b| a.alan.total_cmp(&b.alan))
        .ok_or_else(|| Hata::girdi("seçim", "Kenarın yanında düz bir taban yüzü bulunamadı. Kanal tabanı yüzünü de seçin."))
}

/// The part's long direction in its seat face: its longest straight edge square to the seat
/// normal, pointing along the positive world axis it is closest to.
pub fn uzun_eksen(reply: &Value, normal: V3) -> Result<V3, Hata> {
    let edges = reply.get("edges").and_then(Value::as_array).ok_or_else(|| Hata::komut("model.edges", "Kenarlar okunamadı."))?;
    let mut best: Option<(f64, V3)> = None;
    for e in edges {
        let (Some(a), Some(b)) = (v3(e.get("start")), v3(e.get("end"))) else { continue };
        let chord = b.eksi(a);
        let l = chord.boy();
        let length = e.get("length").and_then(Value::as_f64).unwrap_or(l);
        let Some(dir) = chord.birim() else { continue };
        // Straight (its length is its chord) and lying square to the seat normal.
        if (length - l).abs() > 1e-4 * l.max(1.0) || dir.nokta(normal).abs() > 1e-4 {
            continue;
        }
        if best.is_none_or(|(bl, _)| l > bl + 1e-9) {
            best = Some((l, dir));
        }
    }
    let (_, dir) = best.ok_or_else(|| Hata::girdi("seçim", "Aksesuarın oturma yüzünde düz bir kenar bulunamadı; yönü belirlenemedi."))?;
    let axis = [dir.x, dir.y, dir.z].into_iter().enumerate().max_by(|a, b| a.1.abs().total_cmp(&b.1.abs())).map_or(0.0, |(_, c)| c);
    Ok(if axis < 0.0 { dir.kat(-1.0) } else { dir })
}

/// The planned joint, before occurrences are known.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The corner the distance is measured from.
    pub kose: V3,
    /// The edge's middle (picks it).
    pub kenar_ortasi: V3,
    /// A point on the floor face (picks it).
    pub taban_noktasi: V3,
    /// A point on the seat face (picks it).
    pub oturma_noktasi: V3,
    /// The part's long direction (b's x).
    pub parca_ekseni: V3,
    /// Whether the floor frame's x runs along the edge (else its y does).
    pub x_kenar_boyunca: bool,
    /// The shift across the groove, to the floor's middle (mm).
    pub yanal: f64,
    /// The turn about the floor normal (deg).
    pub aci: f64,
}

pub fn planla(k: &Kenar, taban: &Yuz, oturma: &Yuz, parca_ekseni: V3, sondan: bool, cevir: bool) -> Result<Plan, Hata> {
    let (kose, uzak) = if sondan { (k.son, k.bas) } else { (k.bas, k.son) };
    let de = uzak.eksi(kose).birim().ok_or_else(|| Hata::girdi("seçim", "Seçilen kenarın boyu sıfır."))?;
    let n = taban.normal;
    if n.nokta(de).abs() > 1e-4 || !kenari_tasir(taban, k) {
        return Err(Hata::girdi("seçim", "Seçilen kenar, seçilen kanal tabanı yüzünün üzerinde değil."));
    }
    // Which floor-frame axis the edge is: SolidCraft orders the corner's two edges so that
    // z · (x × y) > 0, and the other edge runs into the face, toward its middle.
    let into = taban.merkez.eksi(kose);
    let triple = n.nokta(de.capraz(into));
    if triple.abs() <= 1e-9 * (into.boy() + 1.0) {
        return Err(Hata::girdi("seçim", "Taban yüzünün ortası kenarın üzerinde kalıyor; kanal tabanı yüzünü ayrıca seçin."));
    }
    let x_kenar_boyunca = triple > 0.0;
    // Across: along y = z × x when x is the edge; along x = edge × z when y is.
    let yanal = if x_kenar_boyunca { into.nokta(n.capraz(de)) } else { into.nokta(de.capraz(n)) };
    let aci = match (x_kenar_boyunca, cevir) {
        (true, false) => 0.0,
        (true, true) => 180.0,
        (false, false) => 90.0,
        (false, true) => -90.0,
    };
    Ok(Plan {
        kose,
        kenar_ortasi: k.bas.arti(k.son).kat(0.5),
        taban_noktasi: taban.nokta,
        oturma_noktasi: oturma.nokta,
        parca_ekseni,
        x_kenar_boyunca,
        yanal: (yanal * 1e6).round() / 1e6,
        aci,
    })
}

impl Plan {
    /// `joint.create`'s parameters.
    pub fn eklem(&self, a: &Value, b: &Value, ad: &str, mesafe: &Mesafe) -> Value {
        let m = mesafe.json();
        let (values, limits) = if self.x_kenar_boyunca {
            (json!([m, self.yanal, self.aci]), json!([null, [self.yanal, self.yanal], [self.aci, self.aci]]))
        } else {
            (json!([self.yanal, m, self.aci]), json!([[self.yanal, self.yanal], null, [self.aci, self.aci]]))
        };
        json!({
            "type": "planar",
            "name": ad,
            "a": {"occurrence": a, "edge": self.kenar_ortasi.json(), "corner": self.kose.json(), "on": self.taban_noktasi.json()},
            "b": {"occurrence": b, "face": self.oturma_noktasi.json(), "x": self.parca_ekseni.json()},
            "flip": false,
            "offset": 0,
            "angle": 0,
            "values": values,
            "limits": limits,
            "quiet": true,
        })
    }
}

/// The occurrence of the component `component.from_bodies` just made, from `component.list`.
fn yeni_yerlesim(liste: &Value, bilesen: u64) -> Option<Value> {
    let c = liste.get("components")?.as_array()?.iter().find(|c| c.get("id").and_then(Value::as_u64) == Some(bilesen))?;
    let o = c.get("occurrences")?.as_array()?.first()?;
    o.get("id").and_then(Value::as_u64).map(|id| json!(id))
}

pub fn calistir(ana: &mut dyn Ana, p: &Value) -> Result<Value, Hata> {
    let metin = secim_metin(p, "mesafe")?.unwrap_or("0");
    let mesafe = ifade::mesafe("mesafe", metin)?;
    let sondan = secim_bool(p, "sondan", false)?;
    let cevir = secim_bool(p, "cevir", false)?;
    if !mesafe.adlar().is_empty() {
        let list = ana.komut("parameters.list", &json!({}))?;
        let names: Vec<String> = list
            .get("parameters")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|p| p.get("name").and_then(Value::as_str).map(str::to_string)).collect())
            .unwrap_or_default();
        ifade::adlar_var_mi("mesafe", &mesafe, &names)?;
    }
    let secim = secimi_oku(&ana.komut("selection.get", &json!({}))?)?;
    let parca_yuzleri = ana.komut("model.faces", &json!({"body": secim.oturma.govde}))?;
    let mut oturma = yuz(&parca_yuzleri, secim.oturma.indeks, "Oturma yüzü")?;
    if let Some(n) = secim.oturma.nokta {
        oturma.nokta = n;
    }
    let kanat_yuzleri = ana.komut("model.faces", &json!({"body": secim.kenar.oge.govde}))?;
    let mut taban = match &secim.taban {
        Some(t) => yuz(&kanat_yuzleri, t.indeks, "Kanal tabanı yüzü")?,
        None => tabani_bul(&kanat_yuzleri, &secim.kenar)?,
    };
    if let Some(n) = secim.taban.as_ref().and_then(|t| t.nokta) {
        taban.nokta = n;
    }
    let kenarlar = ana.komut("model.edges", &json!({"body": secim.oturma.govde}))?;
    let eksen = uzun_eksen(&kenarlar, oturma.normal)?;
    let plan = planla(&secim.kenar, &taban, &oturma, eksen, sondan, cevir)?;

    // The part needs a component of its own to move; a body of the design itself gets one.
    let a = secim.kenar.oge.yerlesim.clone();
    let mut b = secim.oturma.yerlesim.clone();
    let mut bilesen_yapildi = false;
    if b == json!(0) {
        let made = ana.komut("component.from_bodies", &json!({"bodies": [secim.oturma.govde]}))?;
        let id = made
            .pointer("/components/0/component")
            .and_then(Value::as_u64)
            .ok_or_else(|| Hata::komut("component.from_bodies", "Yeni bileşen okunamadı."))?;
        let liste = ana.komut("component.list", &json!({"compact": true}))?;
        b = yeni_yerlesim(&liste, id).ok_or_else(|| Hata::komut("component.list", "Yeni bileşenin yerleşimi bulunamadı."))?;
        bilesen_yapildi = true;
    }
    if a == b {
        return Err(Hata::girdi(
            "seçim",
            "Aksesuar ve kanat aynı bileşende. Aksesuarı ayrı bir bileşen yapın (Gövdelerden Bileşen Oluştur) ve yeniden deneyin.",
        ));
    }
    let ad = format!("Kanal: {}", kisalt(&secim.oturma.govde, 60));
    let reply = ana.komut("joint.create", &plan.eklem(&a, &b, &ad, &mesafe))?;
    let olcu = match &mesafe {
        Mesafe::Sayi(n) => format!("{} mm", crate::json::sayi(*n)),
        Mesafe::Ifade(s) => s.clone(),
    };
    let nereden = if sondan { "sonundan" } else { "başından" };
    let mut mesaj = format!("{} kenarın {nereden} {olcu} uzağa yerleştirildi", secim.oturma.govde);
    if cevir {
        mesaj.push_str(" (180° çevrik)");
    }
    mesaj.push('.');
    if bilesen_yapildi {
        mesaj.push_str(" Aksesuar gövdesi kendi bileşenine alındı.");
    }
    ana.gunluk(&mesaj);
    Ok(json!({
        "mesaj": mesaj,
        "eklem": reply.get("joint").cloned().or_else(|| reply.get("id").cloned()).unwrap_or(Value::Null),
        "ad": ad,
        "uyarilar": reply.get("warnings").cloned().unwrap_or_else(|| json!([])),
    }))
}

#[cfg(test)]
mod tests;
