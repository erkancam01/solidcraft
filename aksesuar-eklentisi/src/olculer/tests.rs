#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::cell::Cell;
use std::rc::Rc;

use serde_json::{Value, json};

use super::*;
use crate::test_ana::TestAna;

fn param(name: &str, expression: &str, value: f64) -> Value {
    json!({"name": name, "expression": expression, "value": value, "unit": if name == "CK" { "" } else { "mm" }, "model": false, "error": null})
}

fn sablon_params() -> Value {
    json!([
        param("FFH", "1500 mm", 1500.0),
        param("FFB", "900 mm", 900.0),
        param("KOL", "600 mm", 600.0),
        param("CK", "0", 0.0),
        param("KA", "120 mm", 120.0)
    ])
}

fn slot(id: &str, label: &str, when: &str, active: bool, placed: bool) -> Value {
    let mut s = json!({"id": id, "label": label, "when": when, "active": active});
    if placed {
        s["placed"] = json!({"slot": id, "path": "x.step", "component": 7, "occurrence": 8, "joint": 9});
    }
    s
}

/// A design made by the template at FFH 1500 with two conditional parts (one made conditional by
/// "Koşullu Parçaları Güncelle", one not) and a conditional slot without a part. The slot list
/// answers as before the change first, then as after FFH 1100.
fn tasarim(params: Value) -> TestAna {
    let listed = Rc::new(Cell::new(0));
    TestAna::new(move |komut, p| {
        Ok(match komut {
            "document.inspect" => json!({"params": params, "metadata": {}}),
            "hardware.slot.list" => {
                let after = listed.get() > 0;
                listed.set(listed.get() + 1);
                json!({"slots": [
                    slot("kol", "Kol", "", true, false),
                    slot("kilit-orta", "Kilit orta", "FFH > 1200 mm", !after, true),
                    slot("kilit-ust", "Kilit üst", "and(FFH > 1400 mm, FFB > 800 mm)", !after, true),
                    slot("ek-kilit-1", "Ek kilit 1", "FFH > 1200 mm", !after, false),
                ]})
            }
            "config.rules" => json!({"rules": [
                {"id": "ek-kilit-buyuk", "effect": {"type": "drive_suppression", "feature": 21, "value": false}},
                {"id": "kosul-kilit-orta-on", "effect": {"type": "drive_suppression", "feature": 60, "value": false}},
                {"id": "kosul-kilit-orta-off", "effect": {"type": "drive_suppression", "feature": 60, "value": true}}
            ]}),
            "parameters.change" => json!({
                "changed": p["changes"].as_object().unwrap().keys().map(|k| (k.clone(), json!(1))).collect::<serde_json::Map<_, _>>(),
                "recomputed": 12,
                "switched": [
                    {"feature": 21, "name": "EK KILIT 1", "suppressed": true},
                    {"feature": 60, "name": "YMB.00146 (2)", "suppressed": true},
                    {"feature": 17, "name": "TIJ sol alt", "suppressed": false}
                ],
                "errors": []
            }),
            other => panic!("not scripted: {other}"),
        })
    })
}

fn ozet(out: &Value) -> String {
    out["ozet"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect::<Vec<_>>().join("\n")
}

#[test]
fn one_size_changes_in_one_edit_and_the_summary_says_what_switched() {
    let mut ana = tasarim(sablon_params());
    let out = calistir(&mut ana, &json!({"FFH": 1100})).unwrap();
    assert_eq!(ana.adlar(), ["document.inspect", "hardware.slot.list", "config.rules", "parameters.change", "hardware.slot.list"]);
    assert_eq!(ana.komutlar[3].1, json!({"changes": {"FFH": "1100 mm"}}));
    assert_eq!(ana.komutlar[0].1, json!({"timeline": false, "bodies": false}));
    let text = ozet(&out);
    assert!(text.contains("• FFH: 1500 → 1100 mm"), "{text}");
    assert!(text.contains("Kurallarla gelen: TIJ sol alt"), "{text}");
    assert!(text.contains("Kurallarla bastırılan: EK KILIT 1, YMB.00146 (2)"), "{text}");
    assert!(text.contains("• Kilit orta: kullanımda → kullanımda değil (parça bastırıldı)"), "{text}");
    assert!(text.contains("• Kilit üst: kullanımda → kullanımda değil — parçası koşullu değil: 'Koşullu Parçaları Güncelle'yi çalıştırın"), "{text}");
    assert!(text.contains("• Ek kilit 1: kullanımda → kullanımda değil") && !text.contains("Ek kilit 1: kullanımda → kullanımda değil ("), "{text}");
    assert!(!text.contains("Kol:"), "an unconditional slot is not listed: {text}");
    assert_eq!(out["degisen"], json!({"FFH": "1100 mm"}));
    assert_eq!(out["noktalar"].as_array().unwrap().len(), 3);
    crate::tests::commands_are_granted("olculer", &ana.adlar());
}

#[test]
fn several_values_and_the_child_lock_go_together() {
    let mut ana = tasarim(sablon_params());
    let out = calistir(&mut ana, &json!({"FFB": 1000, "KOL": 650.5, "cocuk_kilidi": "evet"})).unwrap();
    assert_eq!(ana.komutlar[3].1, json!({"changes": {"FFB": "1000 mm", "KOL": "650.5 mm", "CK": "1"}}));
    let text = ozet(&out);
    assert!(
        text.contains("• FFB: 900 → 1000 mm") && text.contains("• KOL: 600 → 650.5 mm") && text.contains("• CK: 0 → 1 (çocuk kilidi var)"),
        "{text}"
    );
    // A value left as it is is not sent; the child lock "keep" sends nothing.
    let mut ana = tasarim(sablon_params());
    calistir(&mut ana, &json!({"FFH": 1500, "FFB": 950, "cocuk_kilidi": "degistirme"})).unwrap();
    assert_eq!(ana.komutlar[3].1, json!({"changes": {"FFB": "950 mm"}}));
}

#[test]
fn nothing_to_change_changes_nothing() {
    let mut ana = tasarim(sablon_params());
    let out = calistir(&mut ana, &json!({"FFH": 1500, "cocuk_kilidi": "hayir"})).unwrap();
    assert_eq!(ana.adlar(), ["document.inspect"]);
    assert!(ozet(&out).contains("zaten bu değerlerde"));
}

#[test]
fn refusals_are_turkish_and_change_nothing() {
    let why = |params: Value, input: Value| -> (String, Vec<String>) {
        let mut ana = tasarim(params);
        let e = calistir(&mut ana, &input).unwrap_err();
        let neden = match e {
            Hata::Girdi { neden, .. } => neden,
            other => panic!("{other:?}"),
        };
        (neden, ana.adlar().iter().map(|s| s.to_string()).collect())
    };
    // Nothing given: refused before asking the design.
    let (neden, sent) = why(sablon_params(), json!({}));
    assert!(neden.contains("Hiçbir değer girilmedi"), "{neden}");
    assert!(sent.is_empty());
    let (neden, sent) = why(sablon_params(), json!({"cocuk_kilidi": "degistirme"}));
    assert!(neden.contains("Hiçbir değer girilmedi") && sent.is_empty(), "{neden}");
    // A design the template did not make.
    let (neden, sent) = why(json!([param("FFH", "1500 mm", 1500.0)]), json!({"FFH": 1300}));
    assert!(neden.contains("FFB, KOL parametresi yok") && neden.contains("Önce Çift Açılım Şablonu'nu çalıştırın"), "{neden}");
    assert_eq!(sent, ["document.inspect"]);
    let (neden, _) =
        why(json!([param("FFH", "1500 mm", 1500.0), param("FFB", "900 mm", 900.0), param("KOL", "600 mm", 600.0)]), json!({"cocuk_kilidi": "evet"}));
    assert!(neden.contains("CK (çocuk kilidi) parametresi yok"), "{neden}");
    // Out of the dialog's limits, or sizes the template's bars can't take.
    let (neden, sent) = why(sablon_params(), json!({"FFH": 5000}));
    assert!(neden.contains("600 ile 3000 mm") && sent.is_empty(), "{neden}");
    let (neden, sent) = why(sablon_params(), json!({"KOL": 1300}));
    assert!(neden.contains("Kol yüksekliği çok büyük"), "{neden}");
    assert_eq!(sent, ["document.inspect"]);
    let (neden, _) = why(sablon_params(), json!({"FFH": "1500"}));
    assert!(neden.contains("sayı"), "{neden}");
    let (neden, _) = why(sablon_params(), json!({"cocuk_kilidi": "belki"}));
    assert!(neden.contains("Değiştirme, Evet ya da Hayır"), "{neden}");
}

#[test]
fn the_request_reads_optional_fields() {
    assert_eq!(istek(&json!({"FFH": 1300})).unwrap(), Istek { olculer: vec![("FFH", 1300.0)], cocuk_kilidi: None });
    assert_eq!(
        istek(&json!({"KOL": 700, "FFB": null, "cocuk_kilidi": true})).unwrap(),
        Istek { olculer: vec![("KOL", 700.0)], cocuk_kilidi: Some(true) }
    );
    assert_eq!(istek(&json!({"cocuk_kilidi": "hayir"})).unwrap().cocuk_kilidi, Some(false));
    assert!(istek(&json!({"KOL": 272})).is_err());
}
