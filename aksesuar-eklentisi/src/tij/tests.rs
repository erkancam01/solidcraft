#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use serde_json::{Value, json};

use super::*;
use crate::test_ana::TestAna;

fn body(name: &str, min: [f64; 3], max: [f64; 3]) -> Value {
    json!({"name": name, "closed": true, "volume_mm3": 1.0, "area_mm2": 1.0, "center_of_mass": [0, 0, 0],
           "bbox": {"min": min, "max": max}, "faces": 6, "edges": 12, "vertices": 8, "face_types": {}, "kernel": {},
           "measured_by": "smlib", "notes": [], "visible": true})
}

/// `document.inspect {measure: true, timeline: false}` of the template at its defaults.
fn inspect() -> Value {
    json!({
        "name": "Çift açılım", "units": "mm", "params": [
            {"name": "FFH", "expression": "1200 mm", "unit": "mm", "value": 1200.0, "model": false, "comment": "", "error": null},
            {"name": "TIJ_SOL_ALT", "expression": "KOL - IS/2 - KA - 2*PAY", "unit": "mm", "value": 328.0, "model": false, "comment": "", "error": null},
            {"name": "d1", "expression": "5 mm", "unit": "mm", "value": 5.0, "model": true, "comment": "", "error": null},
            {"name": "CK", "expression": "0", "unit": "", "value": 0.0, "model": false, "comment": "", "error": null}
        ],
        "bodies": [
            body("Kanat sol", [0.0, 0.0, -70.0], [62.0, 1200.0, 0.0]),
            body("Kanat sag", [838.0, 0.0, -70.0], [900.0, 1200.0, 0.0]),
            body("Kanat ust", [62.0, 1138.0, -70.0], [838.0, 1200.0, 0.0]),
            body("Kanat alt", [62.0, 0.0, -70.0], [838.0, 62.0, 0.0]),
            body("TIJ sol alt", [-8.5, 15.0, -40.0], [-1.5, 343.0, -30.0]),
            body("TIJ ust", [120.0, 1201.5, -40.0], [478.0, 1208.5, -30.0]),
            body("tij alt", [120.0, -8.5, -40.0], [748.26, -1.5, -30.0]),
            body("KOL rozet", [15.0, 562.0, 0.0], [47.0, 638.0, 14.0]),
            body("KOL tutamak", [21.0, 463.0, 14.0], [41.0, 613.0, 30.0]),
            {"name": "TIJ bozuk", "error": "measure failed", "visible": true},
            body("Cam", [62.0, 62.0, -38.0], [838.0, 1138.0, -32.0])
        ],
        "sketches": [], "selection": [], "undo": 0, "redo": 0, "hidden_origin": []
    })
}

#[test]
fn tie_bars_are_named_tij_in_any_case() {
    for yes in ["TIJ sol alt", "tij", "Tij üst", "TİJ alt", "tıj", "  TIJ_UST"] {
        assert!(tij_mi(yes), "{yes}");
    }
    for no in ["Kanat", "TI", "T IJ", "", "ÇTIJ"] {
        assert!(!tij_mi(no), "{no}");
    }
}

#[test]
fn cut_lengths_round_to_half_millimetres() {
    assert_eq!(kesim_boyu([7.0, 328.0, 10.0]), 328.0);
    assert_eq!(kesim_boyu([628.26, 7.0, 10.0]), 628.5);
    assert_eq!(kesim_boyu([7.0, 10.0, 327.24]), 327.0);
    assert_eq!(kesim_boyu([0.0, 0.0, 0.0]), 0.0);
}

#[test]
fn codes_are_the_name_up_to_the_first_space() {
    assert_eq!(kod("KA-02 dikey"), "KA-02");
    assert_eq!(kod("Cam"), "Cam");
    assert_eq!(kod("  IS-300 ispanyolet "), "IS-300");
    assert_eq!(kod(""), "");
}

#[test]
fn csv_fields_are_safe_for_excel() {
    assert_eq!(alan("Kanat sol"), "Kanat sol");
    assert_eq!(alan("a;b"), "\"a;b\"");
    assert_eq!(alan("15\" profil"), "\"15\"\" profil\"");
    assert_eq!(alan("=HYPERLINK(1)"), "'=HYPERLINK(1)");
    assert_eq!(alan("-5"), "'-5");
    assert_eq!(alan("iki\nsatır"), "iki satır");
}

#[test]
fn file_names_stay_inside_the_granted_folder() {
    assert_eq!(dosya_adi(&json!({})).unwrap(), "tij_listesi.csv");
    assert_eq!(dosya_adi(&json!({"dosya": "  "})).unwrap(), "tij_listesi.csv");
    assert_eq!(dosya_adi(&json!({"dosya": "Kanat 1200x900"})).unwrap(), "Kanat 1200x900.csv");
    assert_eq!(dosya_adi(&json!({"dosya": "liste.CSV"})).unwrap(), "liste.CSV");
    assert_eq!(dosya_adi(&json!({"dosya": "çıktı (2)"})).unwrap(), "çıktı (2).csv");
    for bad in ["../x", "a/b", "a\\b", ".gizli", "C:x", "x\u{0}y"] {
        assert!(matches!(dosya_adi(&json!({"dosya": bad})), Err(Hata::Girdi { .. })), "{bad}");
    }
    assert!(dosya_adi(&json!({"dosya": 5})).is_err());
    assert!(dosya_adi(&json!({"dosya": "a".repeat(101)})).is_err());
}

#[test]
fn the_list_writes_a_turkish_excel_csv_and_a_summary() {
    let mut ana = TestAna::new(|komut, p| {
        assert_eq!(komut, "document.inspect");
        assert_eq!(p, &json!({"measure": true, "timeline": false}));
        Ok(inspect())
    });
    let out = calistir(&mut ana, &json!({})).unwrap();
    crate::tests::commands_are_granted("tij-listesi", &ana.adlar());
    let bytes = ana.dosyalar.get(&("cikti".to_string(), "tij_listesi.csv".to_string())).unwrap();
    assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF], "UTF-8 BOM for Excel");
    let text = std::str::from_utf8(&bytes[3..]).unwrap();
    let expected = "TİJ VE MALZEME LİSTESİ;Çift açılım\r\n\
                    \r\n\
                    TİJLER\r\n\
                    Ad;Kesim boyu (mm)\r\n\
                    TIJ bozuk;ölçülemedi\r\n\
                    TIJ sol alt;328\r\n\
                    TIJ ust;358\r\n\
                    tij alt;628,5\r\n\
                    Toplam;1314,5\r\n\
                    \r\n\
                    PARÇALAR\r\n\
                    Kod;Adet;Gövdeler\r\n\
                    Cam;1;Cam\r\n\
                    KOL;2;KOL rozet, KOL tutamak\r\n\
                    Kanat;4;Kanat sol, Kanat sag, Kanat ust, Kanat alt\r\n\
                    \r\n\
                    PARAMETRELER\r\n\
                    Ad;Değer;Birim;İfade\r\n\
                    FFH;1200;mm;1200 mm\r\n\
                    TIJ_SOL_ALT;328;mm;KOL - IS/2 - KA - 2*PAY\r\n\
                    CK;0;;0\r\n";
    assert_eq!(text, expected);
    assert_eq!(out["dosya"], "cikti/tij_listesi.csv");
    assert_eq!(out["tij_sayisi"], 4);
    assert_eq!(out["toplam_tij_mm"], 1314.5);
    let ozet: Vec<&str> = out["ozet"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect();
    assert_eq!(ozet[0], "4 tij, toplam 1314.5 mm:");
    assert!(ozet.contains(&"  tij alt: 628.5 mm"));
    assert!(ozet.contains(&"  Kanat: 4 adet"));
    assert_eq!(*ozet.last().unwrap(), "Dosya: cikti/tij_listesi.csv");
}

#[test]
fn an_empty_design_writes_nothing() {
    let mut ana = TestAna::new(|_, _| Ok(json!({"name": "Boş", "params": [], "bodies": []})));
    let e = calistir(&mut ana, &json!({})).unwrap_err();
    assert!(matches!(&e, Hata::Girdi { neden, .. } if neden.contains("gövde yok")), "{e:?}");
    assert!(ana.dosyalar.is_empty());
}
