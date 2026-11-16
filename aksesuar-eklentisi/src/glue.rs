//! The component's WIT glue: SolidCraft's published `craft:plugin@0.1.0` world, bound to
//! [`crate::Ana`]. Generated from the host's own WIT file, so a change there breaks this build.

wit_bindgen::generate!({
    path: "../crates/craft-plugin/wit/plugin.wit",
    world: "command-plugin",
});

use craft::plugin::types::{CommandFailure, FieldFailure, PluginError};
use craft::plugin::{commands, files, log};
use serde_json::Value;

use crate::{Ana, Hata};

/// The real host: every call goes through SolidCraft's plugin runtime.
struct Gercek;

fn hata(e: PluginError, komut: &str) -> Hata {
    match e {
        PluginError::PermissionDenied(why) => Hata::Izin(why),
        PluginError::CommandFailed(f) => Hata::komut(f.command, f.reason),
        PluginError::LimitExceeded(why) => Hata::komut(komut, format!("eklenti sınırı aşıldı: {why}")),
        PluginError::InvalidManifest(f) => Hata::komut(komut, format!("geçersiz bildirim {}: {}", f.field, f.reason)),
        PluginError::InvalidInput(f) => Hata::girdi(f.field, f.reason),
        PluginError::Trap(why) => Hata::komut(komut, why),
        PluginError::Cancelled => Hata::Iptal,
    }
}

impl Ana for Gercek {
    fn komut(&mut self, komut: &str, args: &Value) -> Result<Value, Hata> {
        let reply = commands::run(komut, &args.to_string()).map_err(|e| hata(e, komut))?;
        serde_json::from_str(&reply).map_err(|e| Hata::komut(komut, format!("yanıt okunamadı: {e}")))
    }
    fn dosya_yaz(&mut self, izin: &str, yol: &str, veri: &[u8]) -> Result<(), Hata> {
        files::write_file(izin, yol, veri).map_err(|e| match e {
            PluginError::PermissionDenied(why) => Hata::Izin(format!(
                "`{izin}` klasörüne yazılamadı ({why}). Tercihler > Eklentiler'de Aksesuar'ı yeniden yükleyip `{izin}` için bir klasör verin."
            )),
            other => hata(other, "files.write-file"),
        })
    }
    fn gunluk(&mut self, mesaj: &str) {
        log::log(log::Level::Info, mesaj);
    }
}

fn wit_hatasi(h: Hata) -> PluginError {
    match h {
        Hata::Girdi { alan, neden } => PluginError::InvalidInput(FieldFailure { field: alan, reason: neden }),
        Hata::Komut { komut, neden } => PluginError::CommandFailed(CommandFailure { command: komut, reason: neden }),
        Hata::Izin(neden) => PluginError::PermissionDenied(neden),
        Hata::Iptal => PluginError::Cancelled,
    }
}

struct Aksesuar;

impl exports::craft::plugin::entry::Guest for Aksesuar {
    fn execute(entry: String, params: String) -> Result<String, PluginError> {
        let p: Value = serde_json::from_str(&params)
            .map_err(|e| PluginError::InvalidInput(FieldFailure { field: "params".into(), reason: format!("JSON okunamadı: {e}") }))?;
        let out = crate::calistir(&mut Gercek, &entry, &p).map_err(wit_hatasi)?;
        Ok(out.to_string())
    }
}

export!(Aksesuar);
