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
fn the_template_sends_the_reference_design_upright_then_its_locks_bars_and_rules() {
    let mut ana = bos_tasarim(&[]);
    let out = calistir(&mut ana, &json!({"FFH": 1200, "FFB": 900, "KOL": 600})).unwrap();
    let sent = ana.json();
    let reference = reference();
    assert_eq!(reference.len(), 53);
    assert_eq!(sent[0]["command"], "parameters.list");
    let upright: Vec<Value> = reference.iter().map(dik_komut).collect();
    assert_eq!(&sent[1..=53], upright.as_slice(), "the hand-built design's commands, in order, each box stood upright");
    let rest: Vec<&str> = ana.adlar()[54..].to_vec();
    assert_eq!(
        rest,
        [
            "parameters.add",
            "parameters.add",
            "parameters.add",
            "solid.box",
            "appearance.assign",
            "solid.box",
            "appearance.assign",
            "solid.box",
            "appearance.assign",
            "solid.box",
            "appearance.assign",
            "parameters.add",
            "config.configure",
            "config.rule",
            "config.activate",
            "hardware.slot.add",
            "hardware.slot.add",
            "hardware.slot.add",
            "hardware.slot.add",
            "hardware.slot.add",
            "hardware.slot.add",
            "hardware.slot.add",
            "hardware.slot.add",
            "hardware.slot.add"
        ]
    );
    altin("cift_acilim_komutlar.json", &json!(sent));
    // Box ids in reply order: the reference's 20 boxes are 100..=119 ("TIJ sol alt" is the 17th,
    // 116), then the extra lock 120, the split bars 121 and 122, the child lock 123.
    let f = Anahtarlar { ek_kilit: 120, cocuk_kilidi: 123, tij_sol_alt: 116, tij_sol_alt_1: 121, tij_sol_alt_2: 122 };
    assert_eq!(out["ek_kilit"], 120);
    assert_eq!(out["cocuk_kilidi"], 123);
    assert_eq!(out["tij_sol_alt"], 116);
    assert_eq!(out["yerlesim_noktalari"], 9);
    let params = |name: &str| ana.komutlar.iter().find(|(c, _)| c == name).unwrap().1.clone();
    assert_eq!(params("config.configure")["features"], json!(ANAHTARLI));
    assert_eq!(params("config.rule"), kurallar(&f));
    assert_eq!(params("config.activate"), json!({"row": "Default"}));
    let slots: Vec<Value> = ana.komutlar.iter().filter(|(c, _)| c == "hardware.slot.add").map(|(_, p)| p.clone()).collect();
    assert_eq!(slots, yerlesim_noktalari(&Girdi { ffh: 1200.0, ffb: 900.0, kol: 600.0, cocuk_kilidi: false }));
    crate::tests::commands_are_granted("cift-acilim-sablonu", &ana.adlar());
}

#[test]
fn the_slots_sit_on_the_profiles_where_the_template_puts_the_hardware() {
    let g = Girdi { ffh: 1400.0, ffb: 900.0, kol: 600.0, cocuk_kilidi: false };
    let slots = yerlesim_noktalari(&g);
    let slot = |id: &str| slots.iter().find(|s| s["id"] == id).unwrap().clone();
    let ids: Vec<&str> = slots.iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["sol-alt-kose", "sol-ust-kose", "sag-ust-kose", "sag-alt-kose", "alt-mentese", "ust-mentese", "makas", "kol", "ek-kilit-1"]);
    for s in &slots {
        assert_eq!((s["occurrence"].as_u64(), s["across"].as_str(), s["angle"].as_u64()), (Some(0), Some("middle"), Some(0)), "{s}");
        // The corner is an end of the edge through the edge point: the two share all but one coordinate.
        let (c, e) = (s["corner"].as_array().unwrap(), s["edge"].as_array().unwrap());
        assert_eq!(c.iter().zip(e).filter(|(a, b)| a != b).count(), 1, "{s}");
    }
    let groups = |group: &str| slots.iter().filter(|s| s["group"] == group).count();
    assert_eq!((groups(KOSE_AKTARMA), groups(MENTESE), groups(MAKAS_KOL), groups(KILIT)), (4, 2, 2, 1));
    // The four corners on the stiles' outer faces (x = 0 and x = FFB), up the room-side edge.
    assert_eq!(slot("sag-ust-kose")["corner"], json!([900.0, 0.0, 1400.0]));
    assert_eq!(slot("sag-ust-kose")["on"], json!([900.0, 35.0, 700.0]));
    assert_eq!(slot("sol-alt-kose")["edge"], json!([0.0, 0.0, 700.0]));
    // The hinges on the hinge-side stile's ends, the stay on the top rail from its hinge-side end.
    assert_eq!(slot("alt-mentese")["on"], json!([869.0, 35.0, 0.0]));
    assert_eq!(slot("ust-mentese")["corner"], json!([900.0, 0.0, 1400.0]));
    assert_eq!(slot("makas")["corner"], json!([838.0, 0.0, 1400.0]));
    assert_eq!(slot("makas")["edge"], json!([450.0, 0.0, 1400.0]));
    // The handle at KOL on the room face, where the rosette is; the extra lock where its box is.
    assert_eq!((slot("kol")["along"].as_str(), slot("kol")["on"].clone()), (Some("KOL"), json!([31.0, 0.0, 700.0])));
    let ek = slot("ek-kilit-1");
    let lock = govde_komutlari(&g).into_iter().find(|c| c["params"]["name"] == EK_KILIT).unwrap();
    assert_eq!(ek["along"], lock["params"]["center"][2], "the lock box's own height expression");
    assert_eq!(ek["when"], "FFH > 1200 mm");
    assert!(slots.iter().filter(|s| s["id"] != "ek-kilit-1").all(|s| s.get("when").is_none()));
}

#[test]
fn boxes_stand_upright_with_the_room_side_at_minus_y() {
    let (size, center) = dik(["15 mm", "IS", "22 mm"], ["-7.5 mm", "KOL", "-35 mm"]);
    assert_eq!(size, ["15 mm", "22 mm", "IS"]);
    assert_eq!(center, ["-7.5 mm", "35 mm", "KOL"]);
    // The handle rosette was at +Z (room side): it ends up at -Y.
    let (_, rozet) = dik(["32 mm", "76 mm", "14 mm"], ["31 mm", "KOL", "7 mm"]);
    assert_eq!(rozet[1], "-7 mm");
    let (_, genel) = dik(["1 mm", "1 mm", "1 mm"], ["0 mm", "0 mm", "FFB/2"]);
    assert_eq!(genel[1], "-(FFB/2)");
    // Every box the template sends is upright: its height is the old Y size (FFH for the stiles).
    let c = govde_komutlari(&Girdi { ffh: 1200.0, ffb: 900.0, kol: 600.0, cocuk_kilidi: false });
    let sol = c.iter().find(|x| x["params"]["name"] == "Kanat sol").unwrap();
    assert_eq!(sol["params"]["height"], "FFH");
    assert_eq!(sol["params"]["center"], json!(["31 mm", "35 mm", "FFH/2"]));
}

#[test]
fn the_split_bars_fill_the_lower_zone_around_the_extra_lock() {
    let g = Girdi { ffh: 1500.0, ffb: 900.0, kol: 600.0, cocuk_kilidi: false };
    // Zone 120 .. 450 (330 mm); the lock 80 mm in the middle: 125 mm each side, cut 123.
    assert_eq!(tijler(&g), vec![("TIJ_SOL_ALT_1", 123.0), ("TIJ_SOL_ALT_2", 123.0), ("TIJ_SOL_UST", 628.0), ("TIJ_UST", 358.0), ("TIJ_ALT", 628.0)]);
    let c = govde_komutlari(&g);
    let center = |n: &str| c.iter().find(|x| x["params"]["name"] == n).unwrap()["params"]["center"][2].clone();
    assert_eq!(center(EK_KILIT), "(KA + KOL - IS/2)/2");
    assert_eq!(center(TIJ_SOL_ALT_1), "(KA + (KA + KOL - IS/2)/2 - EK/2)/2");
    assert_eq!(center(TIJ_SOL_ALT_2), "((KA + KOL - IS/2)/2 + EK/2 + KOL - IS/2)/2");
    // The same expressions by hand: lock centre 285, bar 1 centre 182.5, bar 2 centre 387.5.
    let (ka, is, ek, kol) = (120.0, 300.0, 80.0, 600.0);
    let lock = (ka + kol - is / 2.0) / 2.0;
    assert_eq!(lock, 285.0);
    assert_eq!((ka + lock - ek / 2.0) / 2.0, 182.5);
    assert_eq!((lock + ek / 2.0 + kol - is / 2.0) / 2.0, 387.5);
    // At or under 1200 mm the bar is whole.
    let kucuk = tijler(&Girdi { ffh: 1200.0, ..g });
    assert_eq!(kucuk[0], ("TIJ_SOL_ALT", 328.0));
}

#[test]
fn the_rules_switch_the_lock_and_bars_together() {
    let f = Anahtarlar { ek_kilit: 7, cocuk_kilidi: 9, tij_sol_alt: 3, tij_sol_alt_1: 4, tij_sol_alt_2: 5 };
    let r = kurallar(&f);
    let rules = r["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 10);
    let effect = |operator: &str, feature: u64| {
        rules
            .iter()
            .find(|x| x["condition"]["parameter"] == "FFH" && x["condition"]["operator"] == operator && x["effect"]["feature"] == feature)
            .unwrap()["effect"]["value"]
            .clone()
    };
    // FFH <= 1200: lock and split bars off, whole bar on; FFH > 1200: the opposite.
    for (feature, kucukte) in [(7, true), (4, true), (5, true), (3, false)] {
        assert_eq!(effect("le", feature), kucukte, "feature {feature} at FFH <= 1200");
        assert_eq!(effect("gt", feature), !kucukte, "feature {feature} at FFH > 1200");
    }
    let ids: Vec<&str> = rules.iter().map(|x| x["id"].as_str().unwrap()).collect();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), ids.len(), "rule ids are unique");
    let off = rules.iter().find(|x| x["id"] == "ek-kilit-kucuk").unwrap();
    assert_eq!(off["condition"], json!({"type": "parameter", "parameter": "FFH", "operator": "le", "expression": "1200 mm"}));
    let ck = rules.iter().find(|x| x["id"] == "cocuk-kilidi-kapali").unwrap();
    assert_eq!(ck["condition"], json!({"type": "parameter", "parameter": "CK", "operator": "eq", "expression": "0"}));
    assert_eq!(ck["effect"], json!({"type": "drive_suppression", "feature": 9, "value": true}));
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
    assert_eq!(t, vec![("TIJ_SOL_ALT", 328.0), ("TIJ_SOL_UST", 328.0), ("TIJ_UST", 358.0), ("TIJ_ALT", 628.0)]);
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
    // With the extra lock (FFH > 1200) each half of the lower bar needs 40 mm: KOL >= 434.
    fails(json!({"FFH": 1500, "KOL": 433}), "KOL", "en az 434");
    let mut ana = bos_tasarim(&[]);
    assert!(calistir(&mut ana, &json!({"FFH": 1500, "KOL": 434})).is_ok());
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
