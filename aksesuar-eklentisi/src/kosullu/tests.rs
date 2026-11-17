#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use serde_json::{Value, json};

use super::*;
use crate::test_ana::{TestAna, altin};

const KA: &str = "C:\\kutuphane\\Köşe aktarma\\YMB.00146 - TIRNAKLI KÖŞE AKTARMA.STEP";
const KILIT: &str = "C:\\kutuphane\\Kilit\\KILIT.STEP";

#[derive(Clone, Debug)]
struct Comp {
    name: String,
    features: Vec<u64>,
    bodies: Vec<String>,
    /// Occurrences inside it: (occurrence, component).
    children: Vec<(u64, u64)>,
}

#[derive(Clone, Debug)]
struct Feat {
    id: u64,
    name: String,
    suppressed: bool,
}

/// A small design that answers like SolidCraft for the commands this entry sends, and refuses
/// what SolidCraft would refuse (a missing feature, parameter or occurrence, a rule on an
/// unconfigured feature): a refused call fails the test's run as it fails the real one.
#[derive(Clone, Debug)]
struct Tasarim {
    slots: Vec<Value>,
    placed: Vec<Value>,
    comps: BTreeMap<u64, Comp>,
    /// Occurrence -> (component, parent component).
    occs: BTreeMap<u64, (u64, u64)>,
    feats: Vec<Feat>,
    /// Joints: (id, name, a, b).
    joints: Vec<(u64, String, u64, u64)>,
    params: BTreeMap<String, String>,
    rules: Vec<Value>,
    columns: BTreeSet<u64>,
    rolled_back: bool,
    next: u64,
}

fn slot(id: &str, label: &str, when: Option<&str>, active: bool) -> Value {
    json!({"id": id, "label": label, "group": "Kilit ispanyolet tij", "along": "FFH/2", "across": "middle", "angle": 0, "flip": false,
        "when": when, "active": active, "frame": {"origin": [0, 35, 650], "x": [0, 0, 1], "y": [0, 1, 0], "z": [-1, 0, 0]},
        "joint_a": {"occurrence": 0, "edge": [0, 0, 650], "corner": [0, 0, 0], "on": [0, 35.8, 34.2]},
        "values": ["FFH/2", 35.0, 0.0], "limits": [null, [35.0, 35.0], [0.0, 0.0]]})
}

fn placed(slot: &str, path: &str, component: u64, occurrence: u64, joint: u64) -> Value {
    json!({"slot": slot, "path": path, "component": component, "occurrence": occurrence, "joint": joint})
}

impl Tasarim {
    /// FFH 1300 x FFB 900: the corner drive STEP placed on an unconditional corner and on the
    /// middle lock (`FFH > 1200 mm`, in use), so both share component 26; a lock STEP on the top
    /// lock (`and(FFH > 1400 mm, FFB > 800 mm)`, not in use); nothing on the extra lock slot.
    fn yeni() -> Self {
        let comps = BTreeMap::from([
            (
                0,
                Comp {
                    name: "Kanat".into(),
                    features: vec![1, 21],
                    bodies: vec!["Kanat sol".into(), "EK KILIT 1".into()],
                    children: vec![(27, 26), (42, 26), (50, 48)],
                },
            ),
            (26, Comp { name: "YMB.00146".into(), features: vec![36], bodies: vec![], children: vec![(29, 28)] }),
            (28, Comp { name: "YMB.00140".into(), features: vec![], bodies: vec!["Scale1".into()], children: vec![] }),
            (48, Comp { name: "KILIT".into(), features: vec![52], bodies: vec![], children: vec![(53, 49)] }),
            (49, Comp { name: "KILIT govde".into(), features: vec![], bodies: vec!["Kilit govde".into()], children: vec![] }),
        ]);
        let occs = BTreeMap::from([(27, (26, 0)), (42, (26, 0)), (29, (28, 26)), (50, (48, 0)), (53, (49, 48))]);
        let feats = vec![
            Feat { id: 1, name: "Kanat sol".into(), suppressed: false },
            Feat { id: 21, name: "EK KILIT 1".into(), suppressed: false },
            Feat { id: 36, name: "YMB.00146 - TIRNAKLI KÖŞE AKTARMA".into(), suppressed: false },
            Feat { id: 52, name: "KILIT".into(), suppressed: false },
        ];
        let template_rule = json!({"id": "ek-kilit-buyuk", "name": "Ek kilit: FFH 1200 mm üstünde var", "enabled": true,
            "condition": {"type": "parameter", "parameter": "FFH", "operator": "gt", "expression": "1200 mm"},
            "effect": {"type": "drive_suppression", "feature": 21, "value": false}});
        Tasarim {
            slots: vec![
                slot("sol-alt-kose", "Sol alt köşe", None, true),
                slot("kilit-orta", "Kilit orta", Some("FFH > 1200 mm"), true),
                slot("kilit-ust", "Kilit üst", Some("and(FFH > 1400 mm, FFB > 800 mm)"), false),
                slot("ek-kilit-1", "Ek kilit 1", Some("FFH > 1200 mm"), true),
            ],
            placed: vec![placed("sol-alt-kose", KA, 26, 27, 41), placed("kilit-orta", KA, 26, 42, 43), placed("kilit-ust", KILIT, 48, 50, 51)],
            comps,
            occs,
            feats,
            joints: vec![
                (37, "Rigid Group 27:29".into(), 27, 29),
                (41, "Sol alt köşe: YMB.00146".into(), 0, 27),
                (43, "Kilit orta: YMB.00146".into(), 0, 42),
                (51, "Kilit üst: KILIT".into(), 0, 50),
            ],
            params: BTreeMap::from([
                ("FFH".into(), "1300 mm".into()),
                ("FFB".into(), "900 mm".into()),
                ("KOL".into(), "600 mm".into()),
                ("CK".into(), "0".into()),
            ]),
            rules: vec![template_rule],
            columns: BTreeSet::from([21]),
            rolled_back: false,
            next: 100,
        }
    }

    fn id(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    fn feat(&self, id: u64) -> Option<&Feat> {
        self.feats.iter().find(|f| f.id == id)
    }

    fn live(&self, p: &Value) -> bool {
        self.occs.get(&p["occurrence"].as_u64().unwrap()).is_some_and(|(c, _)| Some(*c) == p["component"].as_u64())
    }

    fn feature_ids(&self, list: &Value) -> Result<Vec<u64>, String> {
        list.as_array()
            .ok_or("features must be a list")?
            .iter()
            .map(|f| {
                let id = f.as_u64().or_else(|| f.as_str().and_then(|s| s.parse().ok())).ok_or(format!("feature {f}"))?;
                self.feat(id).map(|_| id).ok_or(format!("no feature {id}"))
            })
            .collect()
    }

    fn answer(&mut self, cmd: &str, p: &Value) -> Result<Value, String> {
        Ok(match cmd {
            "hardware.slot.list" => {
                let slots: Vec<Value> = self
                    .slots
                    .iter()
                    .map(|s| {
                        let mut s = s.clone();
                        if let Some(e) = self.placed.iter().find(|e| e["slot"] == s["id"] && self.live(e)) {
                            s["placed"] = e.clone();
                        }
                        s
                    })
                    .collect();
                json!({"slots": slots})
            }
            "config.rules" => {
                let columns: Vec<String> = self.columns.iter().filter_map(|f| self.feat(*f)).map(|f| format!("suppress:{}", f.name)).collect();
                json!({"rules": self.rules, "columns": columns, "rows": [{"id": 1, "name": "Default"}], "active": "Default"})
            }
            "component.list" => {
                let comps: Vec<Value> = self
                    .comps
                    .iter()
                    .map(|(id, c)| {
                        let occurrences: Vec<Value> = self
                            .occs
                            .iter()
                            .filter(|(_, (comp, _))| comp == id)
                            .map(|(o, (comp, parent))| json!({"id": o, "component": comp, "parent": parent}))
                            .collect();
                        let children: Vec<Value> = c.children.iter().map(|(o, comp)| json!({"id": o, "component": comp})).collect();
                        let features: Vec<&str> = c.features.iter().filter_map(|f| self.feat(*f)).map(|f| f.name.as_str()).collect();
                        json!({"id": id, "name": c.name, "features": features, "bodies": c.bodies, "occurrences": occurrences, "children": children})
                    })
                    .collect();
                json!({"active": 0, "components": comps})
            }
            "document.inspect" => {
                let timeline: Vec<Value> = self
                    .feats
                    .iter()
                    .map(|f| {
                        let mut r = json!({"id": f.id, "name": f.name, "type": "BaseFeature"});
                        if f.suppressed {
                            r["suppressed"] = json!(true);
                        }
                        if self.rolled_back && f.id == 52 {
                            r["rolled_back"] = json!(true);
                        }
                        r
                    })
                    .collect();
                // A suppressed feature makes no body; a sub-component's bodies are its parent's feature's.
                let mut bodies = Vec::new();
                for (id, c) in &self.comps {
                    let feature = if *id == 0 {
                        1
                    } else {
                        match self.comps.values().find(|p| p.children.iter().any(|(_, k)| k == id)) {
                            Some(p) => p.features.first().copied().unwrap_or(1),
                            None => continue,
                        }
                    };
                    if self.feat(feature).is_some_and(|f| f.suppressed) {
                        continue;
                    }
                    bodies.extend(c.bodies.iter().map(|b| json!({"name": b, "feature": feature, "visible": true})));
                }
                let params: Vec<Value> = self.params.iter().map(|(n, e)| json!({"name": n, "expression": e, "model": false})).collect();
                json!({"timeline": timeline, "bodies": bodies, "params": params, "marker": self.feats.len(),
                    "metadata": {YERLESIM_ANAHTARI: {"version": 1, "slots": [], "placed": self.placed}, "com.example": {"x": 1}}})
            }
            "joint.list" => {
                json!({"joints": self.joints.iter().map(|(id, n, a, b)| json!({"id": id, "name": n, "a": a, "b": b, "type": "planar"})).collect::<Vec<_>>()})
            }
            "joint.inspect" => {
                let id = p["joint"].as_u64().ok_or("joint")?;
                let (_, name, _, b) = self.joints.iter().find(|j| j.0 == id).ok_or(format!("no joint {id}"))?;
                json!({"joints": [{"id": id, "name": name, "b": {"occurrence": b, "frame": {"origin": [0.0, 35.0, 650.0], "x": [0.0, 0.0, 1.0], "z": [1.0, 0.0, 0.0]}}}]})
            }
            "timeline.suppress" => {
                let ids = self.feature_ids(&p["features"])?;
                let s = p["suppressed"].as_bool().ok_or("suppressed")?;
                for f in self.feats.iter_mut().filter(|f| ids.contains(&f.id)) {
                    f.suppressed = s;
                }
                json!({"features": ids, "suppressed": s})
            }
            "component.paste_new" => {
                if self.rolled_back {
                    return Err("move the timeline marker to the end before Paste New".into());
                }
                let comp = p["component"].as_u64().ok_or("component")?;
                let occ = p["occurrence"].as_u64().ok_or("occurrence")?;
                if self.occs.get(&occ).map(|o| o.0) != Some(comp) {
                    return Err("occurrence belongs to another component".into());
                }
                assert_eq!(p["in_place"], true, "the copy goes where the part is");
                let source = self.comps[&comp].clone();
                if source.features.iter().any(|f| self.feat(*f).unwrap().suppressed) {
                    return Err("the test design copies shown parts only".into());
                }
                let (new_comp, new_occ) = (self.id(), self.id());
                let mut features = Vec::new();
                for f in &source.features {
                    let id = self.id();
                    let name = format!("{} (2)", self.feat(*f).unwrap().name);
                    self.feats.push(Feat { id, name, suppressed: false });
                    features.push(id);
                }
                let mut children = Vec::new();
                for (_, sub) in &source.children {
                    let (c, o) = (self.id(), self.id());
                    let mut copy = self.comps[sub].clone();
                    copy.bodies = copy.bodies.iter().map(|b| format!("{b} (2)")).collect();
                    self.comps.insert(c, copy);
                    self.occs.insert(o, (c, new_comp));
                    children.push((o, c));
                }
                self.comps.insert(new_comp, Comp { name: format!("{} (2)", source.name), features: features.clone(), bodies: vec![], children });
                self.occs.insert(new_occ, (new_comp, 0));
                json!({"component": new_comp, "occurrence": new_occ, "features": features, "components": 2, "bodies": 1, "frozen_bodies": [], "notices": []})
            }
            "selection.delete" => {
                let occs = p["occurrences"].as_array().ok_or("occurrences")?;
                let mut deleted_joints = Vec::new();
                for o in occs {
                    let o = o.as_u64().ok_or("occurrence")?;
                    self.occs.remove(&o).ok_or(format!("no occurrence {o}"))?;
                    deleted_joints.extend(self.joints.iter().filter(|j| j.2 == o || j.3 == o).map(|j| j.0));
                    self.joints.retain(|j| j.2 != o && j.3 != o);
                }
                json!({"deleted": [], "deleted_joints": deleted_joints, "skipped": [], "would_fail": []})
            }
            "joint.create" => {
                let b = p["b"]["occurrence"].as_u64().ok_or("b")?;
                if !self.occs.contains_key(&b) || p["b"]["frame"]["origin"].as_array().map(Vec::len) != Some(3) {
                    return Err("b".into());
                }
                let id = self.id();
                self.joints.push((id, p["name"].as_str().unwrap_or("").to_string(), 0, b));
                json!({"joint": id, "name": p["name"], "errors": [], "conflicts": []})
            }
            "document.metadata.set" => {
                assert_eq!(p["key"], YERLESIM_ANAHTARI);
                self.placed = p["value"]["placed"].as_array().ok_or("placed")?.clone();
                json!({"changed": [YERLESIM_ANAHTARI]})
            }
            "parameters.add" => {
                let name = p["name"].as_str().ok_or("name")?;
                if self.params.contains_key(name) {
                    return Err(format!("parameter `{name}` already exists"));
                }
                assert_eq!(p["unit"], "", "a condition is unitless");
                self.params.insert(name.into(), p["expression"].as_str().ok_or("expression")?.into());
                json!({"name": name, "value": 0.0, "unit": ""})
            }
            "parameters.change" => {
                let name = p["name"].as_str().ok_or("name")?;
                let e = self.params.get_mut(name).ok_or(format!("no parameter `{name}`"))?;
                *e = p["expression"].as_str().ok_or("expression")?.into();
                json!({"name": name})
            }
            "config.configure" => {
                let ids = self.feature_ids(&p["features"])?;
                self.columns.extend(ids);
                json!({"active": "Default", "rows": [{"id": 1, "name": "Default"}]})
            }
            "config.rule" => {
                if let Some(rules) = p.get("rules") {
                    for r in rules.as_array().ok_or("rules")? {
                        let f = r["effect"]["feature"].as_u64().ok_or("feature")?;
                        if !self.columns.contains(&f) {
                            return Err(format!("rule `{}` target is not configured", r["name"]));
                        }
                        let param = r["condition"]["parameter"].as_str().ok_or("parameter")?;
                        if !self.params.contains_key(param) {
                            return Err(format!("references missing parameter `{param}`"));
                        }
                        for k in ["id", "name"] {
                            let n = r[k].as_str().unwrap().chars().count();
                            assert!(n > 0 && n <= if k == "id" { 64 } else { 128 }, "{r}");
                        }
                        match self.rules.iter_mut().find(|x| x["id"] == r["id"]) {
                            Some(x) => *x = r.clone(),
                            None => self.rules.push(r.clone()),
                        }
                    }
                } else if p["delete"] == true {
                    let before = self.rules.len();
                    self.rules.retain(|x| x["id"] != p["id"]);
                    if self.rules.len() == before {
                        return Err("unknown rule".into());
                    }
                } else {
                    return Err("give rule, rules or id".into());
                }
                json!({"rules": self.rules})
            }
            other => return Err(format!("not scripted: {other}")),
        })
    }
}

fn ana(t: &Rc<RefCell<Tasarim>>) -> TestAna {
    let t = Rc::clone(t);
    TestAna::new(move |cmd, p| t.borrow_mut().answer(cmd, p).map_err(|e| Hata::komut(cmd, e)))
}

fn kosul_kurallari(t: &Tasarim) -> Vec<&Value> {
    t.rules.iter().filter(|r| r["id"].as_str().unwrap().starts_with(KURAL_ONEKI)).collect()
}

#[test]
fn conditional_parts_get_rules_and_a_shared_one_its_own_copy() {
    let t = Rc::new(RefCell::new(Tasarim::yeni()));
    let mut a = ana(&t);
    let out = calistir(&mut a, &json!({})).unwrap();
    assert_eq!(
        a.adlar(),
        [
            "hardware.slot.list",
            "config.rules",
            "component.list",
            "document.inspect",
            "joint.list",
            "joint.inspect",
            "component.paste_new",
            "selection.delete",
            "joint.create",
            "document.metadata.set",
            "parameters.add",
            "config.configure",
            "config.rule",
            "timeline.suppress"
        ]
    );
    altin("kosullu_komutlar.json", &json!(a.json()));
    let sent = |name: &str| a.komutlar.iter().find(|(c, _)| c == name).unwrap().1.clone();
    // The middle lock shared component 26 with the corner: it is copied in place, its old
    // occurrence goes (and its joint with it), the same planar joint holds the copy.
    assert_eq!(sent("joint.inspect"), json!({"joint": 43}));
    assert_eq!(sent("component.paste_new"), json!({"component": 26, "occurrence": 42, "in_place": true}));
    assert_eq!(sent("selection.delete"), json!({"occurrences": [42]}));
    let joint = sent("joint.create");
    assert_eq!(joint["type"], "planar");
    assert_eq!(joint["name"], "Kilit orta: YMB.00146");
    assert_eq!(joint["a"], json!({"occurrence": 0, "edge": [0, 0, 650], "corner": [0, 0, 0], "on": [0, 35.8, 34.2]}));
    assert_eq!(joint["b"], json!({"occurrence": 102, "frame": {"origin": [0.0, 35.0, 650.0], "z": [1.0, 0.0, 0.0], "x": [0.0, 0.0, 1.0]}}));
    assert_eq!((joint["values"].clone(), joint["limits"].clone()), (json!(["FFH/2", 35.0, 0.0]), json!([null, [35.0, 35.0], [0.0, 0.0]])));
    assert_eq!((joint["flip"].clone(), joint["quiet"].clone()), (json!(false), json!(true)));
    // The slot list's record points at the copy; the other placements and keys stay.
    let meta = sent("document.metadata.set");
    assert_eq!(meta["value"]["placed"][1], placed("kilit-orta", KA, 101, 102, 106));
    assert_eq!(meta["value"]["placed"][0], placed("sol-alt-kose", KA, 26, 27, 41));
    assert_eq!(meta["value"]["version"], 1);
    // The compound condition gets its parameter; the single comparison is the condition itself.
    assert_eq!(
        sent("parameters.add"),
        json!({"name": "KOSUL_KILIT_UST", "expression": "and(FFH > 1400 mm, FFB > 800 mm)", "unit": "",
            "comment": "Koşul: Kilit üst (Aksesuar, Koşullu Parçaları Güncelle)"})
    );
    assert_eq!(sent("config.configure"), json!({"features": ["103", "52"]}));
    let rules = sent("config.rule")["rules"].as_array().unwrap().clone();
    let ids: Vec<&str> = rules.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["kosul-kilit-orta-on", "kosul-kilit-orta-off", "kosul-kilit-ust-on", "kosul-kilit-ust-off"]);
    assert_eq!(
        rules[0],
        json!({"id": "kosul-kilit-orta-on", "name": "Koşullu parça: Kilit orta: FFH > 1200 mm iken var", "enabled": true,
            "condition": {"type": "parameter", "parameter": "FFH", "operator": "gt", "expression": "1200 mm"},
            "effect": {"type": "drive_suppression", "feature": 103, "value": false}})
    );
    assert_eq!(rules[1]["condition"], json!({"type": "parameter", "parameter": "FFH", "operator": "le", "expression": "1200 mm"}));
    assert_eq!(rules[1]["effect"], json!({"type": "drive_suppression", "feature": 103, "value": true}));
    assert_eq!(rules[2]["condition"], json!({"type": "parameter", "parameter": "KOSUL_KILIT_UST", "operator": "ne", "expression": "0"}));
    assert_eq!(rules[3]["condition"], json!({"type": "parameter", "parameter": "KOSUL_KILIT_UST", "operator": "eq", "expression": "0"}));
    assert_eq!(rules[3]["effect"], json!({"type": "drive_suppression", "feature": 52, "value": true}));
    // Now: the top lock is not in use (FFH 1300), so its part is suppressed; the middle one stays.
    assert_eq!(sent("timeline.suppress"), json!({"features": [52], "suppressed": true}));
    let d = t.borrow();
    assert!(d.feat(52).unwrap().suppressed && !d.feat(103).unwrap().suppressed && !d.feat(36).unwrap().suppressed);
    assert_eq!(d.occs.get(&42), None, "the shared occurrence is gone");
    assert_eq!(d.occs.get(&102), Some(&(101, 0)));
    assert_eq!(d.rules.len(), 5, "the template's rule stays");
    assert_eq!(d.rules[0]["id"], "ek-kilit-buyuk");
    // The summary, in Turkish.
    let ozet = out["ozet"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect::<Vec<_>>().join("\n");
    assert!(ozet.contains("Koşullu parçalar: 2 parça, 4 kural"), "{ozet}");
    assert!(ozet.contains("Kilit orta: koşul `FFH > 1200 mm`, şu an var; aynı bileşeni paylaştığı için kendi kopyasına ayrıldı"), "{ozet}");
    assert!(
        ozet.contains("Kilit üst: koşul `and(FFH > 1400 mm, FFB > 800 mm)`, şu an yok (bastırıldı); koşul parametresi KOSUL_KILIT_UST"),
        "{ozet}"
    );
    assert_eq!(out["parametreler"], json!(["KOSUL_KILIT_UST"]));
    assert_eq!(out["parcalar"][0]["kopya"], true);
    assert_eq!(a.gunluk.len(), 1);
    crate::tests::commands_are_granted("kosullu-parcalar", &a.adlar());
}

#[test]
fn a_second_run_changes_nothing_and_adds_no_rules() {
    let t = Rc::new(RefCell::new(Tasarim::yeni()));
    calistir(&mut ana(&t), &json!({})).unwrap();
    let after_first = t.borrow().rules.clone();
    let mut a = ana(&t);
    let out = calistir(&mut a, &json!({})).unwrap();
    // No copy (nothing is shared now), no parameter (it is there), no suppression (as it is), and
    // the columns and rules are there as they would be made: the design is only read.
    assert_eq!(a.adlar(), ["hardware.slot.list", "config.rules", "component.list", "document.inspect"]);
    assert_eq!(t.borrow().rules, after_first);
    assert_eq!(kosul_kurallari(&t.borrow()).len(), 4);
    assert_eq!(out["parcalar"][0]["kopya"], false);
    assert_eq!(out["parcalar"][0]["yerlesim"], 102);
    // A rule changed by hand is put back, replaced by its id.
    t.borrow_mut().rules[1]["enabled"] = json!(false);
    let mut a = ana(&t);
    calistir(&mut a, &json!({})).unwrap();
    assert_eq!(a.adlar()[4..], ["config.configure", "config.rule"]);
    assert_eq!(t.borrow().rules, after_first);
    assert_eq!(t.borrow().comps.len(), 7, "one copy and its sub-component, made once");
}

#[test]
fn a_slot_that_is_no_longer_conditional_loses_its_rules_and_shows_its_part() {
    let t = Rc::new(RefCell::new(Tasarim::yeni()));
    calistir(&mut ana(&t), &json!({})).unwrap();
    assert!(t.borrow().feat(52).unwrap().suppressed);
    t.borrow_mut().slots[2]["when"] = Value::Null;
    t.borrow_mut().slots[2]["active"] = json!(true);
    let mut a = ana(&t);
    let out = calistir(&mut a, &json!({})).unwrap();
    let deletes: Vec<Value> = a.komutlar.iter().filter(|(c, p)| c == "config.rule" && p.get("delete").is_some()).map(|(_, p)| p.clone()).collect();
    assert_eq!(deletes, [json!({"id": "kosul-kilit-ust-on", "delete": true}), json!({"id": "kosul-kilit-ust-off", "delete": true})]);
    assert_eq!(a.komutlar.last().unwrap(), &("timeline.suppress".to_string(), json!({"features": [52], "suppressed": false})));
    let d = t.borrow();
    assert!(!d.feat(52).unwrap().suppressed);
    let ids: Vec<&str> = kosul_kurallari(&d).iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["kosul-kilit-orta-on", "kosul-kilit-orta-off"]);
    assert_eq!(out["silinen_kurallar"], json!(["kosul-kilit-ust-on", "kosul-kilit-ust-off"]));
}

#[test]
fn without_conditional_parts_nothing_changes() {
    let t = Rc::new(RefCell::new(Tasarim::yeni()));
    t.borrow_mut().placed.retain(|p| p["slot"] == "sol-alt-kose");
    let mut a = ana(&t);
    let out = calistir(&mut a, &json!({})).unwrap();
    assert_eq!(a.adlar(), ["hardware.slot.list", "config.rules"]);
    assert!(out["ozet"][0].as_str().unwrap().contains("yerleştirilmiş parça yok"), "{out}");
    crate::tests::commands_are_granted("kosullu-parcalar", &a.adlar());
}

#[test]
fn a_shared_part_that_cannot_be_copied_is_skipped_and_the_rest_still_done() {
    // A rolled-back timeline: Paste New would be refused, so it is not sent.
    let t = Rc::new(RefCell::new(Tasarim::yeni()));
    t.borrow_mut().rolled_back = true;
    let mut a = ana(&t);
    let out = calistir(&mut a, &json!({})).unwrap();
    assert!(!a.adlar().contains(&"component.paste_new"), "{:?}", a.adlar());
    assert_eq!(out["atlanan"][0]["nokta"], "kilit-orta");
    assert!(out["atlanan"][0]["neden"].as_str().unwrap().contains("işaretçiyi en sona alın"), "{out}");
    let d = t.borrow();
    let ids: Vec<&str> = kosul_kurallari(&d).iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["kosul-kilit-ust-on", "kosul-kilit-ust-off"]);
    drop(d);
    assert!(!t.borrow().feat(36).unwrap().suppressed, "the shared component is left alone");

    // Another joint on the shared occurrence would go with it: skipped too.
    let t = Rc::new(RefCell::new(Tasarim::yeni()));
    t.borrow_mut().joints.push((60, "Kullanıcı ilişkisi".into(), 50, 42));
    let mut a = ana(&t);
    let out = calistir(&mut a, &json!({})).unwrap();
    assert!(!a.adlar().contains(&"component.paste_new"));
    assert!(out["atlanan"][0]["neden"].as_str().unwrap().contains("Kullanıcı ilişkisi"), "{out}");
}

#[test]
fn two_conditional_parts_on_one_component_the_last_keeps_it() {
    // The corner placement made conditional too: component 26 is then used by two conditional
    // parts only, so one copy is enough. The original insert holds the STEP file's rigid group,
    // so the other part is the one copied.
    let t = Rc::new(RefCell::new(Tasarim::yeni()));
    t.borrow_mut().slots[0]["when"] = json!("CK");
    let mut a = ana(&t);
    let out = calistir(&mut a, &json!({})).unwrap();
    assert_eq!(a.adlar().iter().filter(|c| **c == "component.paste_new").count(), 1);
    assert_eq!(a.komutlar.iter().find(|(c, _)| c == "component.paste_new").unwrap().1["occurrence"], 42);
    let parca = |slot: &str| out["parcalar"].as_array().unwrap().iter().find(|p| p["nokta"] == slot).unwrap().clone();
    assert_eq!((parca("sol-alt-kose")["bilesen"].clone(), parca("sol-alt-kose")["kopya"].clone()), (json!(26), json!(false)));
    assert_eq!(parca("kilit-orta")["kopya"], true);
    // A parameter alone is a condition: in use while it is not 0.
    let rules = t.borrow().rules.clone();
    let on = rules.iter().find(|r| r["id"] == "kosul-sol-alt-kose-on").unwrap();
    assert_eq!(on["condition"], json!({"type": "parameter", "parameter": "CK", "operator": "ne", "expression": "0"}));
    assert_eq!(on["effect"]["feature"], 36);
}

#[test]
fn conditions_map_to_rules() {
    let params: BTreeSet<String> = ["FFH", "FFB", "CK", "KOL_2"].iter().map(|s| s.to_string()).collect();
    assert_eq!(karsilastirma("FFH > 1200 mm"), Some(("FFH".into(), "gt", "1200 mm".into())));
    assert_eq!(karsilastirma(" FFB<=800 mm "), Some(("FFB".into(), "le", "800 mm".into())));
    assert_eq!(karsilastirma("KOL_2 != KOL/2 + 1 mm"), Some(("KOL_2".into(), "ne", "KOL/2 + 1 mm".into())));
    assert_eq!(karsilastirma("CK == 1"), Some(("CK".into(), "eq", "1".into())));
    for compound in [
        "and(FFH > 1400 mm, FFB > 800 mm)",
        "FFH/2 > 600 mm",
        "FFH > 1200 mm and FFB > 1",
        "FFH > 1 > 0",
        "FFH > if(CK, 1, 2)",
        "FFH >",
        "> 5",
        "FFH",
    ] {
        assert_eq!(karsilastirma(compound), None, "{compound}");
    }
    assert_eq!(kosul("FFH > 1200 mm", "s", &params), Kosul::Dogrudan { parametre: "FFH".into(), op: "gt", ifade: "1200 mm".into() });
    assert_eq!(kosul("CK", "s", &params), Kosul::Dogrudan { parametre: "CK".into(), op: "ne", ifade: "0".into() });
    // A comparison of a name that is not a parameter goes through a parameter of its own.
    assert_eq!(kosul("pi > 3", "ek-kilit-1", &params), Kosul::Turetilmis { parametre: "KOSUL_EK_KILIT_1".into(), ifade: "pi > 3".into() });
    for (op, ters) in [("gt", "le"), ("ge", "lt"), ("lt", "ge"), ("le", "gt"), ("eq", "ne"), ("ne", "eq")] {
        assert_eq!(tersi(op), ters);
        assert_eq!(tersi(ters), op);
    }
    // SolidCraft's limits: a parameter name and a rule id at most 64 characters.
    let uzun = "a-".repeat(40);
    assert!(kosul_parametresi(&uzun).len() <= 64);
    assert!(kosul_parametresi(&uzun).starts_with("KOSUL_A_A_"));
    let n = Nokta {
        id: uzun.clone(),
        etiket: "Ç".repeat(200),
        kosul: Some("F".repeat(300)),
        aktif: true,
        hata: None,
        eklem_a: None,
        degerler: None,
        sinirlar: None,
        cevir: false,
        parca: None,
    };
    for sira in [0, 1, 11] {
        for r in kurallar(&n, &Kosul::Turetilmis { parametre: "KOSUL_X".into(), ifade: "1".into() }, 7, sira) {
            assert!(r["id"].as_str().unwrap().len() <= 64 && r["name"].as_str().unwrap().chars().count() <= 128, "{r}");
            assert!(noktanin_kurali(r["id"].as_str().unwrap(), &kural_koku(&uzun)), "{r}");
        }
    }
    assert!(noktanin_kurali("kosul-a-on", "kosul-a") && noktanin_kurali("kosul-a-off-2", "kosul-a"));
    assert!(!noktanin_kurali("kosul-a-b-on", "kosul-a") && !noktanin_kurali("kosul-a-on-", "kosul-a") && !noktanin_kurali("kosul-ab-on", "kosul-a"));
}

/// Long slot ids alike at the start get rule ids and condition parameters of their own (a short
/// stable digest of the whole id after a readable start); ids that fit keep the names they had.
#[test]
fn long_slot_ids_alike_at_the_start_never_share_rules_or_parameters() {
    let ortak = "kanat-sag-alt-kose-ek-kilit-ispanyolet-tij-baglanti-";
    let (a, b) = (format!("{ortak}birinci"), format!("{ortak}ikinci"));
    assert!(ortak.len() > 50);
    assert_ne!(kural_koku(&a), kural_koku(&b));
    assert_ne!(kosul_parametresi(&a), kosul_parametresi(&b));
    assert!(kural_koku(&a).starts_with(&format!("kosul-{}-", &a[..40])) && kural_koku(&a).ends_with(&ozet(&a)), "{}", kural_koku(&a));
    assert_eq!(kosul_parametresi(&a), format!("KOSUL_{}_{}", a[..40].to_ascii_uppercase().replace('-', "_"), ozet(&a).to_ascii_uppercase()));
    assert_eq!(ozet(&a), ozet(&a.clone()), "stable");
    assert_eq!(ozet(&a).len(), 8);
    let n = |id: &str| Nokta {
        id: id.into(),
        etiket: "E".into(),
        kosul: Some("FFH > 1 mm".into()),
        aktif: true,
        hata: None,
        eklem_a: None,
        degerler: None,
        sinirlar: None,
        cevir: false,
        parca: None,
    };
    let k = Kosul::Turetilmis { parametre: "KOSUL_X".into(), ifade: "1".into() };
    for sira in [0, 1, 11] {
        for r in kurallar(&n(&a), &k, 7, sira) {
            let id = r["id"].as_str().unwrap();
            assert!(id.len() <= 64 && noktanin_kurallari(id, &a) && !noktanin_kurallari(id, &b), "{r}");
        }
    }
    // 0.1.4 cut such an id to its first 50: those rules are still the slot's (to be replaced).
    let eski = format!("kosul-{}-on", &a[..50]);
    assert!(noktanin_kurallari(&eski, &a));
    // Ids that fit keep their names, so a design made with 0.1.4 stays as it is.
    assert_eq!(kural_koku("kilit-orta"), "kosul-kilit-orta");
    assert_eq!(kosul_parametresi("ek-kilit-1"), "KOSUL_EK_KILIT_1");
    assert_eq!(kural_koku(&"a".repeat(50)), format!("kosul-{}", "a".repeat(50)));
    // Two ids that would share a parameter (`-` and `_` both become `_`): the later one gets its own.
    let adlar = parametre_adlari(["a-b", "a_b", "c"]);
    assert_eq!(adlar["a-b"], "KOSUL_A_B");
    assert_eq!(adlar["c"], "KOSUL_C");
    assert!(adlar["a_b"].starts_with("KOSUL_A_B_") && adlar["a_b"] != adlar["a-b"], "{adlar:?}");
}

/// Only this entry's rules are changed or removed: a rule of the user's own whose id starts with
/// `kosul-` stays as it is; a rule an earlier version made (no `Koşullu parça:` name, the id it
/// makes for a slot of the design) is still this entry's, replaced or removed as its slot is now.
#[test]
fn only_this_entrys_rules_are_changed_or_removed() {
    let t = Rc::new(RefCell::new(Tasarim::yeni()));
    let rule = |id: &str, name: &str, feature: u64| {
        json!({"id": id, "name": name, "enabled": true,
            "condition": {"type": "parameter", "parameter": "FFH", "operator": "gt", "expression": "1 mm"},
            "effect": {"type": "drive_suppression", "feature": feature, "value": false}})
    };
    let user = rule("kosul-benim", "Benim kuralım", 21);
    let named_like_ours = json!({"id": "benim-2", "name": "Koşullu parça: el ile", "enabled": true,
        "condition": {"type": "always"}, "effect": {"type": "drive_suppression", "feature": 21, "value": false}});
    let legacy_now = rule("kosul-kilit-ust-on", "Kilit üst: eski ad iken var", 52);
    let legacy_gone = rule("kosul-sol-alt-kose-off", "Sol alt köşe: CK değilken yok", 36);
    t.borrow_mut().rules.extend([user.clone(), named_like_ours.clone(), legacy_now, legacy_gone]);
    let mut a = ana(&t);
    let out = calistir(&mut a, &json!({})).unwrap();
    let deletes: Vec<Value> = a.komutlar.iter().filter(|(c, p)| c == "config.rule" && p.get("delete").is_some()).map(|(_, p)| p["id"].clone()).collect();
    assert_eq!(deletes, [json!("kosul-sol-alt-kose-off")], "the old rule of a slot that is no longer conditional goes; the user's stay");
    assert_eq!(out["silinen_kurallar"], json!(["kosul-sol-alt-kose-off"]));
    let d = t.borrow();
    assert!(d.rules.contains(&user) && d.rules.contains(&named_like_ours), "{:?}", d.rules);
    let ust: Vec<&Value> = d.rules.iter().filter(|r| r["id"] == "kosul-kilit-ust-on").collect();
    assert_eq!(ust.len(), 1);
    assert_eq!(ust[0]["name"], "Koşullu parça: Kilit üst: and(FFH > 1400 mm, FFB > 800 mm) iken var", "replaced by id");
    assert!(bizim_kural(ust[0], &[]), "this entry's by its name alone");
    assert!(!bizim_kural(&user, &noktalar(&json!({"slots": d.slots}))) && !bizim_kural(&named_like_ours, &[]));
}

#[test]
fn hostile_replies_are_read_without_panicking() {
    for reply in [json!(null), json!([]), json!({"slots": 3}), json!({"slots": [1, {"id": 5}, {"id": "x", "placed": {"component": "a"}}]})] {
        let n = noktalar(&reply);
        assert!(n.iter().all(|n| n.parca.is_none()));
    }
    assert!(bilesenler(&json!({"components": [{"id": 1, "children": [3, {"id": "x"}], "occurrences": "no"}]})).contains_key(&1));
    let cycle = BTreeMap::from([
        (1, Bilesen { icindekiler: vec![(5, Some(2))], ..Default::default() }),
        (2, Bilesen { icindekiler: vec![(6, Some(1))], ..Default::default() }),
    ]);
    assert_eq!(agac(&cycle, 1), [1, 2]);
    assert!(ozellik_idleri(&cycle, &[1, 2], &[], &json!({})).is_err());
    assert!(eklemler(&json!({"joints": [{"id": "x"}, {"id": 4}]})).len() == 1);
}
