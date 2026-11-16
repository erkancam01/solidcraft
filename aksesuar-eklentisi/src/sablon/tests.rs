#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::cell::Cell;
use std::rc::Rc;

use serde_json::{Value, json};

use super::*;
use crate::test_ana::{TestAna, altin};

/// A host with an empty design: boxes get feature ids 100, 101, ...
fn bos_tasarim(existing: &'static [&'static str]) -> TestAna {
    let next = Rc::new(Cell::new(100u64));
    TestAna::new(move |komut, p| {
        Ok(match komut {
            "parameters.list" => json!({"parameters": existing.iter().map(|n| json!({"name": n, "expression": "1 mm"})).collect::<Vec<_>>()}),
            "solid.box" => {
                let id = next.get();
                next.set(id + 1);
                json!({"feature": id, "name": p["name"], "bodies": [], "created_bodies": [p["body_name"]], "changed_bodies": [], "removed_bodies": [], "warning": null})
            }
            "config.configure" => json!({"columns": [], "rows": [{"id": 1, "name": "Default"}], "active": "Default", "matches": true}),
            _ => json!({}),
        })
    })
}

fn reference() -> Vec<Value> {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/veri/cift_acilim_referans.json")).unwrap();
    let v: Value = serde_json::from_str(&text).unwrap();
    v["commands"].as_array().unwrap().clone()
}

#[test]
fn the_template_sends_the_reference_design_unchanged_then_its_locks_and_rules() {
    let mut ana = bos_tasarim(&[]);
    let out = calistir(&mut ana, &json!({"FFH": 1200, "FFB": 900, "KOL": 600})).unwrap();
    let sent = ana.json();
    let reference = reference();
    assert_eq!(reference.len(), 53);
    assert_eq!(sent[0]["command"], "parameters.list");
    assert_eq!(&sent[1..=53], reference.as_slice(), "the ported geometry is the hand-built design's, command for command");
    let rest: Vec<&str> = ana.adlar()[54..].to_vec();
    assert_eq!(
        rest,
        ["solid.box", "appearance.assign", "solid.box", "appearance.assign", "parameters.add", "config.configure", "config.rule", "config.activate"]
    );
    altin("cift_acilim_komutlar.json", &json!(sent));
    // Box ids in reply order: the reference's 20 boxes are 100..=119, the locks 120 and 121.
    assert_eq!(out["ek_kilit"], 120);
    assert_eq!(out["cocuk_kilidi"], 121);
    let rule = &ana.komutlar[ana.komutlar.len() - 2].1;
    assert_eq!(rule, &kurallar(120, 121));
    assert_eq!(ana.komutlar.last().unwrap().1, json!({"row": "Default"}));
    crate::tests::commands_are_granted("cift-acilim-sablonu", &ana.adlar());
}

#[test]
fn the_rules_switch_each_lock_both_ways() {
    let r = kurallar(7, 9);
    let rules = r["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 4);
    let find = |id: &str| rules.iter().find(|x| x["id"] == id).unwrap().clone();
    let off = find("ek-kilit-kapali");
    assert_eq!(off["condition"], json!({"type": "parameter", "parameter": "FFH", "operator": "le", "expression": "1200 mm"}));
    assert_eq!(off["effect"], json!({"type": "drive_suppression", "feature": 7, "value": true}));
    assert_eq!(find("ek-kilit-acik")["condition"]["operator"], "gt");
    assert_eq!(find("ek-kilit-acik")["effect"]["value"], false);
    assert_eq!(find("cocuk-kilidi-kapali")["condition"], json!({"type": "parameter", "parameter": "CK", "operator": "eq", "expression": "0"}));
    assert_eq!(find("cocuk-kilidi-kapali")["effect"], json!({"type": "drive_suppression", "feature": 9, "value": true}));
    assert_eq!(find("cocuk-kilidi-acik")["effect"]["value"], false);
    for x in rules {
        // config_rules.rs refuses unknown fields: only these keys.
        let mut keys: Vec<&String> = x.as_object().unwrap().keys().collect();
        keys.sort();
        assert_eq!(keys, ["condition", "effect", "enabled", "id", "name"]);
    }
}

#[test]
fn the_child_lock_box_follows_the_checkbox() {
    let ck = |on: bool| {
        govde_komutlari(&Girdi { ffh: 1400.0, ffb: 1100.0, kol: 700.0, cocuk_kilidi: on }).into_iter().find(|c| c["params"]["name"] == "CK").unwrap()
            ["params"]
            .clone()
    };
    assert_eq!(ck(true), json!({"name": "CK", "expression": "1", "unit": "", "comment": "Çocuk kilidi: 1 var, 0 yok"}));
    assert_eq!(ck(false)["expression"], "0");
    let c = govde_komutlari(&Girdi { ffh: 1234.5, ffb: 900.0, kol: 600.0, cocuk_kilidi: false });
    assert_eq!(c[0]["params"], json!({"name": "FFH", "expression": "1234.5 mm"}));
}

#[test]
fn the_scissor_and_tie_bars_match_the_parameter_formulas() {
    assert_eq!(makas(400.0), 220.0);
    assert_eq!(makas(650.0), 220.0);
    assert_eq!(makas(650.5), 420.0);
    assert_eq!(makas(1000.0), 420.0);
    assert_eq!(makas(1001.0), 650.0);
    let t = tijler(&Girdi { ffh: 1200.0, ffb: 900.0, kol: 600.0, cocuk_kilidi: false });
    assert_eq!(t, [("TIJ_SOL_ALT", 328.0), ("TIJ_SOL_UST", 328.0), ("TIJ_UST", 358.0), ("TIJ_ALT", 628.0)]);
}

#[test]
fn inputs_are_checked_before_anything_is_sent() {
    let fails = |p: Value, alan: &str, says: &str| {
        let mut ana = bos_tasarim(&[]);
        match calistir(&mut ana, &p) {
            Err(Hata::Girdi { alan: a, neden }) => {
                assert_eq!(a, alan, "{neden}");
                assert!(neden.contains(says), "{neden}");
            }
            other => panic!("{p}: {other:?}"),
        }
        assert!(ana.komutlar.is_empty(), "{p}");
    };
    fails(json!({"KOL": 250}), "KOL", "en az 273");
    fails(json!({"FFH": 800, "KOL": 600}), "KOL", "en çok 527");
    fails(json!({"FFH": 100}), "FFH", "600 ile 3000");
    fails(json!({"FFB": "geniş"}), "FFB", "sayı");
    fails(json!({"cocuk_kilidi": "evet"}), "cocuk_kilidi", "true/false");
    fails(json!({"FFH": f64::MAX}), "FFH", "arasında");
}

#[test]
fn a_design_that_has_the_parameters_is_refused() {
    let mut ana = bos_tasarim(&["Genislik", "KOL"]);
    let e = calistir(&mut ana, &json!({})).unwrap_err();
    assert!(matches!(&e, Hata::Girdi { neden, .. } if neden.contains("`KOL`") && neden.contains("boş")), "{e:?}");
    assert_eq!(ana.adlar(), ["parameters.list"]);
}

#[test]
fn a_box_reply_without_a_feature_id_stops_the_template() {
    let mut ana = TestAna::new(|komut, _| Ok(if komut == "parameters.list" { json!({"parameters": []}) } else { json!({}) }));
    let e = calistir(&mut ana, &json!({})).unwrap_err();
    assert!(matches!(&e, Hata::Komut { komut, .. } if komut == "solid.box"), "{e:?}");
    assert!(!ana.adlar().contains(&"config.rule"));
}
