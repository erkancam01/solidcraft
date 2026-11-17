//! "Koşullu Parçaları Güncelle": makes the parts placed on conditional placement slots come and
//! go with their slot's condition.
//!
//! A slot's `when` (e.g. `FFH > 1200 mm`) says whether it is in use; `hardware.place` puts a part
//! there regardless. For every such slot with a part, this entry adds two configuration rules
//! (`drive_suppression`, SolidCraft's own) on the part's Import feature: shown while the condition
//! holds, suppressed otherwise. An ordinary parameter edit then runs them, so the part follows the
//! condition without the plugin. The part's joint keeps working while it is suppressed: its part
//! side is a frame, not a face of the part.
//!
//! - A `when` that is one comparison of a parameter (`FFH > 1200 mm`) is the rules' condition
//!   itself. Anything else (`and(FFH > 1400 mm, FFB > 800 mm)`) goes into a unitless parameter
//!   `KOSUL_<SLOT>` the rules compare with 0.
//! - `hardware.place` places a STEP file it already brought in as another occurrence of the same
//!   component, so two placements would share one Import feature. A conditional part that shares
//!   its component is made independent first: a copy of the component in the same place
//!   (`component.paste_new`), the old occurrence deleted (its joint with it), the same planar
//!   joint made again on the copy, and the slot list's `placed` entry pointed at the copy.
//! - The rules have fixed ids (`kosul-<slot>-on`/`-off`), so a second run replaces them; rules of
//!   this entry whose slot is no longer conditional are removed and their part shown again.
//!
//! Every command is checked before it is sent: a refused host call ends the whole run.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

use crate::json::kisalt;
use crate::{Ana, Hata};

/// The design metadata key `hardware.place` keeps the slots and their parts under.
pub const YERLESIM_ANAHTARI: &str = "tr.aksesuar.yerlesim";
/// The ids of this entry's rules start with this.
pub const KURAL_ONEKI: &str = "kosul-";
/// The names of this entry's rules start with this: a rule whose id starts with
/// [`KURAL_ONEKI`] is this entry's when its name does too (or, from before 0.1.5, when its id is
/// one this entry makes for a slot of the design).
pub const AD_ONEKI: &str = "Koşullu parça: ";
/// The derived condition parameters' names start with this.
pub const PARAMETRE_ONEKI: &str = "KOSUL_";
/// SolidCraft's limits: a rule id at most 64 characters, a parameter name at most 64.
const EN_UZUN_KURAL: usize = 64;
const EN_UZUN_PARAMETRE: usize = 64;
/// How much of a slot id goes into a rule id (`kosul-` + this + `-off-NN`).
const KURAL_KOKU: usize = EN_UZUN_KURAL - KURAL_ONEKI.len() - 8;
/// A longer slot id goes in cut to this, then `-` and its [`ozet`].
const OKUNUR: usize = 40;

/// A short stable digest of `metin`: 8 hex digits of its 32-bit FNV-1a hash. A long slot id cut
/// to fit a rule id or a parameter name keeps it, so two ids alike at the start stay apart.
pub fn ozet(metin: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for b in metin.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    format!("{h:08x}")
}

/// What `hardware.place` put on a slot: its component, occurrence and joint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Yerlesik {
    pub bilesen: u64,
    pub yerlesim: u64,
    pub eklem: u64,
}

/// A placement slot, as `hardware.slot.list` gives it.
#[derive(Clone, Debug, PartialEq)]
pub struct Nokta {
    pub id: String,
    pub etiket: String,
    /// Its `when`, trimmed; `None` when it is always in use.
    pub kosul: Option<String>,
    pub aktif: bool,
    pub hata: Option<String>,
    /// `joint.create`'s `a`, `values` and `limits` that put a part there now.
    pub eklem_a: Option<Value>,
    pub degerler: Option<Value>,
    pub sinirlar: Option<Value>,
    pub cevir: bool,
    pub parca: Option<Yerlesik>,
}

fn u64_of(v: &Value, key: &str) -> Option<u64> {
    v.get(key).and_then(Value::as_u64)
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// The slots of a `hardware.slot.list` reply.
pub fn noktalar(reply: &Value) -> Vec<Nokta> {
    let Some(slots) = reply.get("slots").and_then(Value::as_array) else { return Vec::new() };
    slots
        .iter()
        .filter_map(|s| {
            let id = str_of(s, "id")?.to_string();
            let parca = s
                .get("placed")
                .filter(|p| p.is_object())
                .and_then(|p| Some(Yerlesik { bilesen: u64_of(p, "component")?, yerlesim: u64_of(p, "occurrence")?, eklem: u64_of(p, "joint")? }));
            Some(Nokta {
                etiket: str_of(s, "label").map(str::trim).filter(|l| !l.is_empty()).unwrap_or(&id).to_string(),
                kosul: str_of(s, "when").map(str::trim).filter(|w| !w.is_empty()).map(str::to_string),
                aktif: s.get("active").and_then(Value::as_bool).unwrap_or(false),
                hata: str_of(s, "error").map(str::to_string),
                eklem_a: s.get("joint_a").filter(|v| v.is_object()).cloned(),
                degerler: s.get("values").filter(|v| v.is_array()).cloned(),
                sinirlar: s.get("limits").filter(|v| v.is_array()).cloned(),
                cevir: s.get("flip").and_then(Value::as_bool).unwrap_or(false),
                id,
                parca,
            })
        })
        .collect()
}

/// A rule condition's operator for a comparison symbol.
fn operator(sembol: &str) -> Option<&'static str> {
    Some(match sembol {
        ">" => "gt",
        ">=" => "ge",
        "<" => "lt",
        "<=" => "le",
        "==" => "eq",
        "!=" => "ne",
        _ => return None,
    })
}

/// The operator that holds exactly when `op` does not.
pub fn tersi(op: &str) -> &'static str {
    match op {
        "gt" => "le",
        "ge" => "lt",
        "lt" => "ge",
        "le" => "gt",
        "eq" => "ne",
        _ => "eq",
    }
}

/// `when` as one comparison of a parameter, `<NAME> <op> <expression>`: the name, the rule
/// operator and the expression. `None` for anything else (a comparison of an expression, `and`,
/// `or`, `not`, `if`, more than one comparison).
pub fn karsilastirma(when: &str) -> Option<(String, &'static str, String)> {
    let t = when.trim();
    let ad_boyu =
        t.char_indices().take_while(|(i, c)| if *i == 0 { c.is_ascii_alphabetic() } else { c.is_ascii_alphanumeric() || *c == '_' }).count();
    let ad = t.get(..ad_boyu).filter(|a| !a.is_empty())?;
    let kalan = t.get(ad_boyu..)?.trim_start();
    let sembol = [">=", "<=", "==", "!=", ">", "<"].into_iter().find(|s| kalan.starts_with(s))?;
    let ifade = kalan.get(sembol.len()..)?.trim();
    if ifade.is_empty() || ifade.contains(['<', '>', '=', '!', '&', '|', '?', ':', ';']) {
        return None;
    }
    let mantik = ifade
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .any(|w| ["and", "or", "not", "if", "true", "false"].contains(&w.to_ascii_lowercase().as_str()));
    if mantik {
        return None;
    }
    Some((ad.to_string(), operator(sembol)?, ifade.to_string()))
}

/// The derived condition parameter's name for a slot: `KOSUL_` and the slot id in capitals,
/// `-` and anything else not a letter or digit as `_`; one that would be over 64 characters,
/// the id's first 40 and `_` and the whole id's [`ozet`] in capitals.
pub fn kosul_parametresi(slot: &str) -> String {
    let kok: String = slot.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
    let ad = format!("{PARAMETRE_ONEKI}{kok}");
    if ad.len() <= EN_UZUN_PARAMETRE { ad } else { ozel_parametre(slot) }
}

/// A slot's condition parameter name that no other slot's is: `KOSUL_`, the id's first 40 in
/// capitals and `_` and its [`ozet`] in capitals.
fn ozel_parametre(slot: &str) -> String {
    let kok: String = slot.chars().take(OKUNUR).map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
    format!("{PARAMETRE_ONEKI}{kok}_{}", ozet(slot).to_ascii_uppercase())
}

/// The condition parameter of each of `slots` (ids, in the slot list's order): its
/// [`kosul_parametresi`], or, when an earlier slot's is the same (`a-b` and `a_b`), its own
/// [`ozel_parametre`], so no two slots share one.
pub fn parametre_adlari<'a>(slots: impl IntoIterator<Item = &'a str>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut alinan = BTreeSet::new();
    for slot in slots {
        if out.contains_key(slot) {
            continue;
        }
        let mut ad = kosul_parametresi(slot);
        if !alinan.insert(ad.clone()) {
            ad = ozel_parametre(slot);
            alinan.insert(ad.clone());
        }
        out.insert(slot.to_string(), ad);
    }
    out
}

/// How a slot's condition reaches the rules.
#[derive(Clone, Debug, PartialEq)]
pub enum Kosul {
    /// One comparison of a parameter, or a parameter alone (in use while it is not 0).
    Dogrudan { parametre: String, op: &'static str, ifade: String },
    /// Anything else, kept in a unitless parameter that is 1 while it holds and 0 otherwise.
    Turetilmis { parametre: String, ifade: String },
}

impl Kosul {
    /// The condition of the rule that shows the part (`var`) or suppresses it.
    pub fn kosul(&self, var: bool) -> Value {
        let (parametre, op, ifade) = match self {
            Kosul::Dogrudan { parametre, op, ifade } => (parametre.as_str(), *op, ifade.as_str()),
            Kosul::Turetilmis { parametre, .. } => (parametre.as_str(), "ne", "0"),
        };
        json!({"type": "parameter", "parameter": parametre, "operator": if var { op } else { tersi(op) }, "expression": ifade})
    }
}

/// `when` of slot `slot` for the rules. `parametreler`: the design's parameter names.
pub fn kosul(when: &str, slot: &str, parametreler: &BTreeSet<String>) -> Kosul {
    kosul_adiyla(when, &kosul_parametresi(slot), parametreler)
}

/// [`kosul`], a derived condition kept in the parameter `ad`.
pub fn kosul_adiyla(when: &str, ad: &str, parametreler: &BTreeSet<String>) -> Kosul {
    let t = when.trim();
    if parametreler.contains(t) {
        return Kosul::Dogrudan { parametre: t.to_string(), op: "ne", ifade: "0".into() };
    }
    match karsilastirma(t) {
        Some((parametre, op, ifade)) if parametreler.contains(&parametre) => Kosul::Dogrudan { parametre, op, ifade },
        _ => Kosul::Turetilmis { parametre: ad.to_string(), ifade: t.to_string() },
    }
}

/// The root of a slot's rule ids: `kosul-<slot id>`; an id too long for SolidCraft's 64, its
/// first 40 characters and `-` and the whole id's [`ozet`], so two long ids alike at the start
/// get rules of their own.
pub fn kural_koku(slot: &str) -> String {
    if slot.chars().count() <= KURAL_KOKU {
        format!("{KURAL_ONEKI}{slot}")
    } else {
        format!("{KURAL_ONEKI}{}-{}", slot.chars().take(OKUNUR).collect::<String>(), ozet(slot))
    }
}

/// The root 0.1.4 and before gave a slot's rule ids: the id cut to fit (the same as
/// [`kural_koku`]'s for an id that fits).
fn eski_kural_koku(slot: &str) -> String {
    format!("{KURAL_ONEKI}{}", slot.chars().take(KURAL_KOKU).collect::<String>())
}

/// Whether rule `id` is one of [`kurallar`]'s for the slot whose rule id root is `kok`.
pub fn noktanin_kurali(id: &str, kok: &str) -> bool {
    let Some(kalan) = id.strip_prefix(kok) else { return false };
    ["-on", "-off"].iter().any(|ek| {
        kalan
            .strip_prefix(ek)
            .is_some_and(|n| n.is_empty() || n.strip_prefix('-').is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())))
    })
}

/// Whether rule `id` is one this entry makes, or made before 0.1.5, for slot `slot`.
pub fn noktanin_kurallari(id: &str, slot: &str) -> bool {
    noktanin_kurali(id, &kural_koku(slot)) || noktanin_kurali(id, &eski_kural_koku(slot))
}

/// Whether a rule (as `config.rules` lists it) is this entry's: its id starts with
/// [`KURAL_ONEKI`] and its name with [`AD_ONEKI`]; or, made before 0.1.5 (no such name), its id is
/// exactly one this entry makes for one of `slots`. A rule of the user's own whose id happens to
/// start with `kosul-` is never changed or removed.
pub fn bizim_kural(kural: &Value, slots: &[Nokta]) -> bool {
    let Some(id) = str_of(kural, "id").filter(|id| id.starts_with(KURAL_ONEKI)) else { return false };
    str_of(kural, "name").is_some_and(|ad| ad.starts_with(AD_ONEKI)) || slots.iter().any(|n| noktanin_kurallari(id, &n.id))
}

/// The two rules for feature `ozellik` of slot `n`'s part (`sira`: the feature's place among the
/// part's features, from 0; the first one's ids carry no number).
pub fn kurallar(n: &Nokta, k: &Kosul, ozellik: u64, sira: usize) -> [Value; 2] {
    let kok = kural_koku(&n.id);
    let ek = if sira == 0 { String::new() } else { format!("-{}", sira + 1) };
    // Within SolidCraft's 128 characters: the prefix, 33, 6, 57 and the rest.
    let kosul_metni = kisalt(n.kosul.as_deref().unwrap_or(""), 56);
    let etiket = kisalt(&n.etiket, 32);
    let ad_ek = if sira == 0 { String::new() } else { format!(" ({})", sira + 1) };
    let rule = |var: bool| {
        let (id, ad) = if var {
            (format!("{kok}-on{ek}"), format!("{AD_ONEKI}{etiket}{ad_ek}: {kosul_metni} iken var"))
        } else {
            (format!("{kok}-off{ek}"), format!("{AD_ONEKI}{etiket}{ad_ek}: {kosul_metni} değilken yok"))
        };
        json!({"id": id, "name": ad, "enabled": true, "condition": k.kosul(var),
            "effect": {"type": "drive_suppression", "feature": ozellik, "value": !var}})
    };
    [rule(true), rule(false)]
}

/// A component, as `component.list` gives it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bilesen {
    pub ad: String,
    /// Its features, by name.
    pub ozellikler: Vec<String>,
    pub govdeler: Vec<String>,
    /// Its own occurrences (where it is placed).
    pub yerlesimler: Vec<u64>,
    /// The occurrences inside it: (occurrence, its component when the list says).
    pub icindekiler: Vec<(u64, Option<u64>)>,
}

/// The components of a `component.list` reply, by id.
pub fn bilesenler(reply: &Value) -> BTreeMap<u64, Bilesen> {
    let names = |v: Option<&Value>| -> Vec<String> {
        v.and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
    };
    let mut out = BTreeMap::new();
    for c in reply.get("components").and_then(Value::as_array).into_iter().flatten() {
        let Some(id) = u64_of(c, "id") else { continue };
        let yerlesimler =
            c.get("occurrences").and_then(Value::as_array).map(|a| a.iter().filter_map(|o| u64_of(o, "id")).collect()).unwrap_or_default();
        let icindekiler = c
            .get("children")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|o| o.as_u64().map(|id| (id, None)).or_else(|| Some((u64_of(o, "id")?, u64_of(o, "component"))))).collect())
            .unwrap_or_default();
        out.insert(
            id,
            Bilesen {
                ad: str_of(c, "name").unwrap_or("").to_string(),
                ozellikler: names(c.get("features")),
                govdeler: names(c.get("bodies")),
                yerlesimler,
                icindekiler,
            },
        );
    }
    out
}

/// Component `kok` and every component inside it, once each, `kok` first.
pub fn agac(bilesenler: &BTreeMap<u64, Bilesen>, kok: u64) -> Vec<u64> {
    let mut out = vec![kok];
    let mut i = 0;
    while let Some(id) = out.get(i).copied() {
        i += 1;
        for (_, alt) in bilesenler.get(&id).map(|b| b.icindekiler.as_slice()).unwrap_or_default() {
            if let Some(alt) = alt
                && !out.contains(alt)
            {
                out.push(*alt);
            }
        }
    }
    out
}

/// A timeline row of `document.inspect {timeline: "summary"}`.
#[derive(Clone, Debug, PartialEq)]
pub struct Satir {
    pub id: u64,
    pub ad: String,
    pub bastirili: bool,
    pub hatali: bool,
    pub geride: bool,
}

pub fn satirlar(belge: &Value) -> Vec<Satir> {
    belge
        .get("timeline")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| {
            Some(Satir {
                id: u64_of(r, "id")?,
                ad: str_of(r, "name").unwrap_or("").to_string(),
                bastirili: r.get("suppressed").and_then(Value::as_bool).unwrap_or(false),
                hatali: r.get("error").is_some_and(|e| !e.is_null()),
                geride: r.get("rolled_back").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

/// The feature ids of the components `agac` (a part's component and those inside it): each
/// feature name from `component.list` found on the timeline, a name on several rows told apart
/// by the bodies it made (`document.inspect`'s `bodies[].feature`).
pub fn ozellik_idleri(bilesenler: &BTreeMap<u64, Bilesen>, agac: &[u64], satirlar: &[Satir], belge: &Value) -> Result<Vec<u64>, String> {
    let govdeler: BTreeSet<&str> = agac.iter().filter_map(|c| bilesenler.get(c)).flat_map(|b| b.govdeler.iter().map(String::as_str)).collect();
    let yapanlar: BTreeSet<u64> = belge
        .get("bodies")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|b| str_of(b, "name").is_some_and(|n| govdeler.contains(n)))
        .filter_map(|b| u64_of(b, "feature"))
        .collect();
    let mut out = Vec::new();
    for ad in agac.iter().filter_map(|c| bilesenler.get(c)).flat_map(|b| b.ozellikler.iter()) {
        let adaylar: Vec<u64> = satirlar.iter().filter(|s| &s.ad == ad).map(|s| s.id).collect();
        let id = match adaylar.as_slice() {
            [id] => *id,
            [] => return Err(format!("`{}` özelliği zaman çizelgesinde bulunamadı", kisalt(ad, 60))),
            _ => match adaylar.iter().filter(|id| yapanlar.contains(id)).collect::<Vec<_>>().as_slice() {
                [id] => **id,
                _ => return Err(format!("`{}` adında birden çok özellik var; hangisi olduğu anlaşılamadı", kisalt(ad, 60))),
            },
        };
        if !out.contains(&id) {
            out.push(id);
        }
    }
    if out.is_empty() {
        return Err("parçanın bastırılabilecek bir özelliği yok".into());
    }
    Ok(out)
}

/// A joint of `joint.list`: id, name and the occurrences on its two sides (0: the design).
#[derive(Clone, Debug, PartialEq)]
pub struct Eklem {
    pub id: u64,
    pub ad: String,
    pub a: Option<u64>,
    pub b: Option<u64>,
}

pub fn eklemler(reply: &Value) -> Vec<Eklem> {
    reply
        .get("joints")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|j| Some(Eklem { id: u64_of(j, "id")?, ad: str_of(j, "name").unwrap_or("").to_string(), a: u64_of(j, "a"), b: u64_of(j, "b") }))
        .collect()
}

/// The rules of a `config.rules` reply: id and the feature it switches, if it does.
pub fn mevcut_kurallar(reply: &Value) -> Vec<(String, Option<u64>)> {
    reply
        .get("rules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|r| Some((str_of(r, "id")?.to_string(), r.pointer("/effect/feature").and_then(Value::as_u64))))
        .collect()
}

/// Whether `eski` (as `config.rules` lists it) is the rule `yeni` already: the same id, name,
/// switch, condition and effect (SolidCraft adds its own `length_unit`).
pub fn ayni_kural(eski: &Value, yeni: &Value) -> bool {
    ["id", "name", "enabled", "condition", "effect"].iter().all(|k| eski.get(*k) == yeni.get(*k))
}

/// The design's parameters (all of them, those dimensions make too): name and expression.
pub fn parametreler(belge: &Value) -> BTreeMap<String, String> {
    belge
        .get("params")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| Some((str_of(p, "name")?.to_string(), str_of(p, "expression").unwrap_or("").to_string())))
        .collect()
}

/// A conditional part this run handles.
#[derive(Clone, Debug, PartialEq)]
struct Is {
    nokta: Nokta,
    parca: Yerlesik,
    ozellikler: Vec<u64>,
    kopya: bool,
}

/// Why a part sharing its component can't be made independent now, or the joint that holds it.
fn ayrilabilir(n: &Nokta, parca: &Yerlesik, ic: &BTreeSet<u64>, eklemler: &[Eklem], geride: bool) -> Result<Eklem, String> {
    if geride {
        return Err("zaman çizelgesi geri alınmış; işaretçiyi en sona alın".into());
    }
    if n.eklem_a.is_none() || n.degerler.is_none() || n.sinirlar.is_none() {
        return Err("yerleşim noktası şu an bir parça alamıyor".into());
    }
    let eklem = eklemler.iter().find(|e| e.id == parca.eklem).cloned().ok_or("parçanın yerleşim ilişkisi bulunamadı")?;
    let yabanci: Vec<&str> = eklemler
        .iter()
        .filter(|e| e.id != parca.eklem)
        .filter(|e| {
            let (a, b) = (e.a.unwrap_or(0), e.b.unwrap_or(0));
            (a == parca.yerlesim && !ic.contains(&b)) || (b == parca.yerlesim && !ic.contains(&a))
        })
        .map(|e| e.ad.as_str())
        .collect();
    if !yabanci.is_empty() {
        return Err(format!("parçaya başka ilişkiler de bağlı ({})", kisalt(&yabanci.join(", "), 120)));
    }
    Ok(eklem)
}

/// Points `placed` entry of slot `slot` (occurrence `eski`) at the copy.
fn yerlesimi_guncelle(kayit: &mut Value, slot: &str, eski: u64, yeni: Yerlesik) -> bool {
    let Some(placed) = kayit.get_mut("placed").and_then(Value::as_array_mut) else { return false };
    let Some(e) = placed.iter_mut().find(|e| str_of(e, "slot") == Some(slot) && u64_of(e, "occurrence") == Some(eski)) else { return false };
    let Some(m) = e.as_object_mut() else { return false };
    m.insert("component".into(), json!(yeni.bilesen));
    m.insert("occurrence".into(), json!(yeni.yerlesim));
    m.insert("joint".into(), json!(yeni.eklem));
    true
}

fn durum_metni(aktif: bool) -> &'static str {
    if aktif { "şu an var" } else { "şu an yok (bastırıldı)" }
}

pub fn calistir(ana: &mut dyn Ana, _p: &Value) -> Result<Value, Hata> {
    let liste = ana.komut("hardware.slot.list", &json!({}))?;
    let tum = noktalar(&liste);
    let kural_cevabi = ana.komut("config.rules", &json!({}))?;
    // This entry's rules only: never a rule of the user's own whose id starts the same way.
    let eski_kurallar: Vec<(String, Option<u64>)> = kural_cevabi
        .get("rules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|r| bizim_kural(r, &tum))
        .filter_map(|r| Some((str_of(r, "id")?.to_string(), r.pointer("/effect/feature").and_then(Value::as_u64))))
        .collect();
    // Each conditional slot's own condition parameter, never another slot's.
    let parametre_adi = parametre_adlari(tum.iter().filter(|n| n.kosul.is_some()).map(|n| n.id.as_str()));
    let kosul_of = |n: &Nokta, adlar: &BTreeSet<String>| {
        let ad = parametre_adi.get(&n.id).cloned().unwrap_or_else(|| kosul_parametresi(&n.id));
        kosul_adiyla(n.kosul.as_deref().unwrap_or(""), &ad, adlar)
    };
    let adaylar: Vec<&Nokta> = tum.iter().filter(|n| n.kosul.is_some() && n.parca.is_some()).collect();
    if adaylar.is_empty() && eski_kurallar.is_empty() {
        return Ok(json!({"ozet": [
            "Koşullu bir yerleşim noktasında (koşulu olan, ör. FFH > 1200 mm) yerleştirilmiş parça yok; bir şey değişmedi.",
            "Parçayı Aksesuar Kütüphanesi'nden koşullu bir noktaya yerleştirip bu komutu yeniden çalıştırın."
        ], "parcalar": [], "kurallar": 0}));
    }
    let bilesen_cevabi = ana.komut("component.list", &json!({}))?;
    let bilesenler = bilesenler(&bilesen_cevabi);
    let belge = ana.komut("document.inspect", &json!({"timeline": "summary", "bodies": true}))?;
    let satirlar = satirlar(&belge);
    let geride = satirlar.iter().any(|s| s.geride);
    let mut durum: BTreeMap<u64, bool> = satirlar.iter().map(|s| (s.id, s.bastirili)).collect();
    let mut adet: BTreeMap<u64, usize> = bilesenler.iter().map(|(id, b)| (*id, b.yerlesimler.len())).collect();

    // The parts to handle, and those that can't be, with why.
    let mut isler: Vec<(Is, Vec<u64>)> = Vec::new();
    let mut atlanan: Vec<(Nokta, String)> = Vec::new();
    for n in adaylar {
        let Some(parca) = n.parca else { continue };
        if let Some(h) = &n.hata {
            atlanan.push((n.clone(), format!("yerleşim noktası şu an hesaplanamıyor: {}", kisalt(h, 120))));
            continue;
        }
        if !bilesenler.contains_key(&parca.bilesen) {
            atlanan.push((n.clone(), "parçanın bileşeni bulunamadı".into()));
            continue;
        }
        let agac = agac(&bilesenler, parca.bilesen);
        match ozellik_idleri(&bilesenler, &agac, &satirlar, &belge) {
            Ok(ozellikler) => isler.push((Is { nokta: n.clone(), parca, ozellikler, kopya: false }, agac)),
            Err(e) => atlanan.push((n.clone(), e)),
        }
    }

    // Parts sharing their component: each but the last of a component's conditional parts (all
    // of them when it is also placed unconditionally) becomes a copy. Those whose occurrence holds
    // the STEP file's own rigid groups go last, so the original keeps them.
    let paylasilan = isler.iter().any(|(is, _)| adet.get(&is.parca.bilesen).copied().unwrap_or(0) > 1);
    let eklemler = if paylasilan { eklemler(&ana.komut("joint.list", &json!({}))?) } else { Vec::new() };
    let ici = |agac: &[u64]| -> BTreeSet<u64> {
        agac.iter().filter_map(|c| bilesenler.get(c)).flat_map(|b| b.icindekiler.iter().map(|(o, _)| *o)).collect()
    };
    let grubu_var = |is: &Is, agac: &[u64]| {
        let ic = ici(agac);
        eklemler.iter().any(|e| {
            e.id != is.parca.eklem && (e.a == Some(is.parca.yerlesim) || e.b == Some(is.parca.yerlesim)) && {
                let other = if e.a == Some(is.parca.yerlesim) { e.b } else { e.a };
                other.is_some_and(|o| ic.contains(&o))
            }
        })
    };
    let sira: Vec<&str> = tum.iter().map(|n| n.id.as_str()).collect();
    let mut sirali: Vec<(Is, Vec<u64>, bool)> = isler
        .into_iter()
        .map(|(is, agac)| {
            let grup = grubu_var(&is, &agac);
            (is, agac, grup)
        })
        .collect();
    sirali.sort_by_key(|(_, _, grup)| *grup);

    let mut kayit: Option<Value> = belge.pointer(&format!("/metadata/{YERLESIM_ANAHTARI}")).filter(|v| v.is_object()).cloned();
    let mut kayit_degisti = false;
    let mut hazir: Vec<Is> = Vec::new();
    for (mut is, agac, _) in sirali {
        let c = is.parca.bilesen;
        if adet.get(&c).copied().unwrap_or(0) <= 1 {
            hazir.push(is);
            continue;
        }
        let eklem = match ayrilabilir(&is.nokta, &is.parca, &ici(&agac), &eklemler, geride) {
            Ok(e) => e,
            Err(e) => {
                atlanan.push((is.nokta.clone(), format!("parça başka yerleşimlerle aynı bileşeni paylaşıyor ve ayrılamadı: {e}")));
                continue;
            }
        };
        if kayit.is_none() {
            atlanan.push((is.nokta.clone(), "tasarımın yerleşim kaydı okunamadı".into()));
            continue;
        }
        let ji = ana.komut("joint.inspect", &json!({"joint": eklem.id}))?;
        let cerceve = ji.pointer("/joints/0/b/frame");
        let eksen = |k: &str| cerceve.and_then(|f| f.get(k)).filter(|v| v.as_array().is_some_and(|a| a.len() == 3)).cloned();
        let (Some(merkez), Some(x), Some(z)) = (eksen("origin"), eksen("x"), eksen("z")) else {
            return Err(Hata::komut(
                "joint.inspect",
                format!("`{}` ilişkisinin parça tarafı okunamadı; parçayı yeniden yerleştirin.", kisalt(&eklem.ad, 60)),
            ));
        };
        // A suppressed part is copied shown: its bodies are what the copy is made of.
        let gizli: Vec<u64> = is.ozellikler.iter().copied().filter(|f| durum.get(f).copied().unwrap_or(false)).collect();
        if !gizli.is_empty() {
            ana.komut("timeline.suppress", &json!({"features": gizli, "suppressed": false}))?;
            for f in &gizli {
                durum.insert(*f, false);
            }
        }
        let kopya = ana.komut("component.paste_new", &json!({"component": c, "occurrence": is.parca.yerlesim, "in_place": true}))?;
        let (Some(yeni_bilesen), Some(yeni_yerlesim)) = (u64_of(&kopya, "component"), u64_of(&kopya, "occurrence")) else {
            return Err(Hata::komut("component.paste_new", "Parçanın kopyası okunamadı."));
        };
        let yeni_ozellikler: Vec<u64> =
            kopya.get("features").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
        if yeni_ozellikler.is_empty() {
            return Err(Hata::komut("component.paste_new", "Parçanın kopyasında özellik yok; bastırılamaz."));
        }
        ana.komut("selection.delete", &json!({"occurrences": [is.parca.yerlesim]}))?;
        let b = json!({"occurrence": yeni_yerlesim, "frame": {"origin": merkez, "z": z, "x": x}});
        let yapilan = ana.komut(
            "joint.create",
            &json!({"type": "planar", "name": eklem.ad, "a": is.nokta.eklem_a, "b": b, "values": is.nokta.degerler,
                "limits": is.nokta.sinirlar, "flip": is.nokta.cevir, "offset": 0, "angle": 0, "quiet": true}),
        )?;
        let yeni_eklem = u64_of(&yapilan, "joint").ok_or_else(|| Hata::komut("joint.create", "Yeni ilişki okunamadı."))?;
        let yeni = Yerlesik { bilesen: yeni_bilesen, yerlesim: yeni_yerlesim, eklem: yeni_eklem };
        if let Some(k) = kayit.as_mut() {
            kayit_degisti |= yerlesimi_guncelle(k, &is.nokta.id, is.parca.yerlesim, yeni);
        }
        if let Some(n) = adet.get_mut(&c) {
            *n -= 1;
        }
        for f in &yeni_ozellikler {
            durum.insert(*f, false);
        }
        is.parca = yeni;
        is.ozellikler = yeni_ozellikler;
        is.kopya = true;
        hazir.push(is);
    }
    if kayit_degisti && let Some(k) = kayit {
        ana.komut("document.metadata.set", &json!({"key": YERLESIM_ANAHTARI, "value": k}))?;
    }
    // Back in the slot list's order, for the rules and the summary.
    hazir.sort_by_key(|is| sira.iter().position(|id| *id == is.nokta.id));

    // The derived condition parameters.
    let mut mevcut = parametreler(&belge);
    let adlar: BTreeSet<String> = mevcut.keys().cloned().collect();
    let mut kurallar_yeni: Vec<Value> = Vec::new();
    let mut eklenen_parametreler = Vec::new();
    let mut tablo: Vec<String> = Vec::new();
    for is in &hazir {
        let k = kosul_of(&is.nokta, &adlar);
        if let Kosul::Turetilmis { parametre, ifade } = &k {
            match mevcut.get(parametre) {
                Some(e) if e.trim() == ifade => {}
                Some(_) => {
                    ana.komut("parameters.change", &json!({"name": parametre, "expression": ifade}))?;
                    mevcut.insert(parametre.clone(), ifade.clone());
                    eklenen_parametreler.push(parametre.clone());
                }
                None => {
                    ana.komut(
                        "parameters.add",
                        &json!({"name": parametre, "expression": ifade, "unit": "",
                            "comment": format!("Koşul: {} (Aksesuar, Koşullu Parçaları Güncelle)", kisalt(&is.nokta.etiket, 60))}),
                    )?;
                    mevcut.insert(parametre.clone(), ifade.clone());
                    eklenen_parametreler.push(parametre.clone());
                }
            }
        }
        for (i, f) in is.ozellikler.iter().enumerate() {
            kurallar_yeni.extend(kurallar(&is.nokta, &k, *f, i));
            tablo.push(f.to_string());
        }
    }

    // This entry's rules from an earlier run whose slot is no longer conditional go, and their
    // part is shown again; those of slots skipped now stay as they are.
    let yeni_idler: BTreeSet<&str> = kurallar_yeni.iter().filter_map(|r| str_of(r, "id")).collect();
    let yeni_ozellikler: BTreeSet<u64> = hazir.iter().flat_map(|is| is.ozellikler.iter().copied()).collect();
    let korunan: Vec<&str> = atlanan.iter().map(|(n, _)| n.id.as_str()).collect();
    let mut silinen = Vec::new();
    let mut goster: Vec<u64> = Vec::new();
    for (id, ozellik) in &eski_kurallar {
        if yeni_idler.contains(id.as_str()) || korunan.iter().any(|slot| noktanin_kurallari(id, slot)) {
            continue;
        }
        ana.komut("config.rule", &json!({"id": id, "delete": true}))?;
        silinen.push(id.clone());
        if let Some(f) = ozellik
            && !yeni_ozellikler.contains(f)
            && durum.get(f).copied().unwrap_or(false)
            && !goster.contains(f)
        {
            goster.push(*f);
        }
    }

    // A run that finds every column and rule in place sends neither (and makes no undo step).
    let sutunlar: BTreeSet<&str> = kural_cevabi.get("columns").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    let sutun_var = |f: &str| {
        let ad = f.parse::<u64>().ok().and_then(|id| satirlar.iter().find(|s| s.id == id)).map(|s| format!("suppress:{}", s.ad));
        ad.is_some_and(|ad| sutunlar.contains(ad.as_str()))
    };
    let eski_tam: Vec<&Value> = kural_cevabi.get("rules").and_then(Value::as_array).into_iter().flatten().collect();
    let kurallar_hazir = kurallar_yeni.iter().all(|r| eski_tam.iter().any(|e| ayni_kural(e, r)));
    if !kurallar_yeni.is_empty() && !(kurallar_hazir && tablo.iter().all(|f| sutun_var(f))) {
        ana.komut("config.configure", &json!({"features": tablo}))?;
        ana.komut("config.rule", &json!({"rules": kurallar_yeni}))?;
    }
    // The parts as their slots are now: the rules run on the next parameter edit.
    let mut gizle: Vec<u64> = Vec::new();
    for is in &hazir {
        for f in &is.ozellikler {
            let istenen = !is.nokta.aktif;
            if durum.get(f).copied().unwrap_or(false) != istenen {
                if istenen {
                    gizle.push(*f)
                } else {
                    goster.push(*f)
                }
                durum.insert(*f, istenen);
            }
        }
    }
    if !gizle.is_empty() {
        ana.komut("timeline.suppress", &json!({"features": gizle, "suppressed": true}))?;
    }
    if !goster.is_empty() {
        ana.komut("timeline.suppress", &json!({"features": goster, "suppressed": false}))?;
    }

    let mut ozet = Vec::new();
    if hazir.is_empty() {
        ozet.push("Koşullu yapılabilen parça yok.".to_string());
    } else {
        ozet.push(format!("Koşullu parçalar: {} parça, {} kural (Tasarım Tablosu > Konfigürasyon kuralları).", hazir.len(), kurallar_yeni.len()));
    }
    let mut parcalar = Vec::new();
    for is in &hazir {
        let k = kosul_of(&is.nokta, &adlar);
        let mut satir = format!(
            "• {}: koşul `{}`, {}",
            kisalt(&is.nokta.etiket, 60),
            kisalt(is.nokta.kosul.as_deref().unwrap_or(""), 80),
            durum_metni(is.nokta.aktif)
        );
        if let Kosul::Turetilmis { parametre, .. } = &k {
            satir.push_str(&format!("; koşul parametresi {parametre}"));
        }
        if is.kopya {
            satir.push_str("; aynı bileşeni paylaştığı için kendi kopyasına ayrıldı");
        }
        ozet.push(satir);
        parcalar.push(json!({"nokta": is.nokta.id, "kosul": is.nokta.kosul, "aktif": is.nokta.aktif, "ozellikler": is.ozellikler,
            "bilesen": is.parca.bilesen, "yerlesim": is.parca.yerlesim, "eklem": is.parca.eklem, "kopya": is.kopya,
            "parametre": match &k { Kosul::Turetilmis { parametre, .. } => json!(parametre), Kosul::Dogrudan { .. } => Value::Null }}));
    }
    for (n, neden) in &atlanan {
        ozet.push(format!("• {}: atlandı — {neden}", kisalt(&n.etiket, 60)));
    }
    if !silinen.is_empty() {
        ozet.push(format!("Koşulu kalmayan noktaların eski kuralları silindi: {}.", silinen.join(", ")));
    }
    if !hazir.is_empty() {
        ozet.push(
            "Ölçüleri 'Ölçüleri Değiştir' ile ya da Global Değişkenler panelinden değiştirin; parçalar koşullarına göre kendiliğinden gelip gider."
                .into(),
        );
    }
    ana.gunluk(&format!("koşullu parçalar: {} parça, {} kural, {} atlandı", hazir.len(), kurallar_yeni.len(), atlanan.len()));
    let mut out = Map::new();
    out.insert("ozet".into(), json!(ozet));
    out.insert("parcalar".into(), json!(parcalar));
    out.insert("kurallar".into(), json!(kurallar_yeni.len()));
    out.insert("parametreler".into(), json!(eklenen_parametreler));
    out.insert("silinen_kurallar".into(), json!(silinen));
    out.insert("atlanan".into(), json!(atlanan.iter().map(|(n, neden)| json!({"nokta": n.id, "neden": neden})).collect::<Vec<_>>()));
    Ok(Value::Object(out))
}

#[cfg(test)]
mod tests;
