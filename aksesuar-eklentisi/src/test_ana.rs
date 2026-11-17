//! A scripted host for tests: records every command and file, answers from a closure.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::{Ana, Hata};

type Yanit = Box<dyn FnMut(&str, &Value) -> Result<Value, Hata>>;

pub struct TestAna {
    pub komutlar: Vec<(String, Value)>,
    pub dosyalar: BTreeMap<(String, String), Vec<u8>>,
    pub gunluk: Vec<String>,
    yanit: Yanit,
}

impl TestAna {
    pub fn new(yanit: impl FnMut(&str, &Value) -> Result<Value, Hata> + 'static) -> Self {
        TestAna { komutlar: Vec::new(), dosyalar: BTreeMap::new(), gunluk: Vec::new(), yanit: Box::new(yanit) }
    }
    /// The commands sent, as `{command, params}` objects.
    pub fn json(&self) -> Vec<Value> {
        self.komutlar.iter().map(|(c, p)| json!({"command": c, "params": p})).collect()
    }
    pub fn adlar(&self) -> Vec<&str> {
        self.komutlar.iter().map(|(c, _)| c.as_str()).collect()
    }
}

impl Ana for TestAna {
    fn komut(&mut self, komut: &str, args: &Value) -> Result<Value, Hata> {
        self.komutlar.push((komut.to_string(), args.clone()));
        (self.yanit)(komut, args)
    }
    fn dosya_yaz(&mut self, izin: &str, yol: &str, veri: &[u8]) -> Result<(), Hata> {
        self.dosyalar.insert((izin.to_string(), yol.to_string()), veri.to_vec());
        Ok(())
    }
    fn gunluk(&mut self, mesaj: &str) {
        self.gunluk.push(mesaj.to_string());
    }
}

/// Compares `actual` with the golden file `tests/veri/<name>`; `AKSESUAR_BLESS=1` rewrites it.
pub fn altin(name: &str, actual: &Value) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/veri").join(name);
    let pretty = serde_json::to_string_pretty(actual).unwrap_or_default() + "\n";
    if std::env::var_os("AKSESUAR_BLESS").is_some() {
        std::fs::write(&path, pretty).unwrap_or_default();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_default();
    let expected: Value = serde_json::from_str(&expected).unwrap_or(Value::Null);
    assert_eq!(&expected, actual, "golden {name} differs; rerun with AKSESUAR_BLESS=1 after checking the change:\n{pretty}");
}
