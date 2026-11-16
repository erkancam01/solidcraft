//! Aksesuar: window-hardware helpers for SolidCraft, as one `commands@0.1` plugin.
//!
//! Every entry is plain Rust over the [`Ana`] trait (the host: SolidCraft's command surface and
//! the granted folders), so it runs and is tested natively against a scripted host. The WIT glue
//! that binds the trait to the real host lives in `glue` and is compiled for wasm32 only.
//! User-facing text is Turkish; code and comments are English.
#![cfg_attr(not(any(target_arch = "wasm32", feature = "wit-kontrol")), forbid(unsafe_code))]

pub mod hata;
pub mod ifade;
pub mod json;
pub mod kanal;
pub mod ozellikler;
pub mod sablon;
pub mod tij;
pub mod vektor;

#[cfg(any(target_arch = "wasm32", feature = "wit-kontrol"))]
mod glue;

use serde_json::Value;

pub use hata::Hata;

/// The manifest shipped with the component.
pub const MANIFEST: &str = include_str!("../plugin.json");

/// The entries, by their manifest names.
pub const GIRISLER: &[&str] = &["cift-acilim-sablonu", "kanala-yerlestir", "ozellikler", "tij-listesi"];

/// What a plugin entry can reach: SolidCraft's commands and the folders the user granted.
pub trait Ana {
    /// Runs one SolidCraft command with JSON arguments and returns its JSON reply.
    fn komut(&mut self, komut: &str, args: &Value) -> Result<Value, Hata>;
    /// Writes `veri` to `yol` inside the granted folder `izin`.
    fn dosya_yaz(&mut self, izin: &str, yol: &str, veri: &[u8]) -> Result<(), Hata>;
    /// A line for the host's log.
    fn gunluk(&mut self, _mesaj: &str) {}
}

/// Runs entry `giris` with its JSON parameters.
pub fn calistir(ana: &mut dyn Ana, giris: &str, parametreler: &Value) -> Result<Value, Hata> {
    let bos = Value::Object(serde_json::Map::new());
    let p = if parametreler.is_null() { &bos } else { parametreler };
    if !p.is_object() {
        return Err(Hata::girdi("params", "Girdiler bir JSON nesnesi olmalı."));
    }
    match giris {
        "kanala-yerlestir" => kanal::calistir(ana, p),
        "tij-listesi" => tij::calistir(ana, p),
        "cift-acilim-sablonu" => sablon::calistir(ana, p),
        "ozellikler" => Ok(ozellikler::sonuc()),
        other => Err(Hata::girdi("entry", format!("Bilinmeyen komut: `{}`.", json::kisalt(other, 64)))),
    }
}

#[cfg(test)]
mod test_ana;
#[cfg(test)]
mod tests;
