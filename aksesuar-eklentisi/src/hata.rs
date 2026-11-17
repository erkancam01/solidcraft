//! The plugin's errors: what went wrong, in Turkish, for the user's dialog.

use std::fmt;

/// An entry's failure. Each maps onto the WIT `plugin-error` case of the same meaning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hata {
    /// An input (a field, or the selection) is not usable.
    Girdi { alan: String, neden: String },
    /// A SolidCraft command refused or failed.
    Komut { komut: String, neden: String },
    /// A permission is missing (a folder, a command group).
    Izin(String),
    /// The user pressed Stop.
    Iptal,
}

impl Hata {
    pub fn girdi(alan: impl Into<String>, neden: impl Into<String>) -> Self {
        Hata::Girdi { alan: alan.into(), neden: neden.into() }
    }
    pub fn komut(komut: impl Into<String>, neden: impl Into<String>) -> Self {
        Hata::Komut { komut: komut.into(), neden: neden.into() }
    }
}

impl fmt::Display for Hata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Hata::Girdi { alan, neden } => write!(f, "{alan}: {neden}"),
            Hata::Komut { komut, neden } => write!(f, "{komut} komutu başarısız: {neden}"),
            Hata::Izin(neden) => write!(f, "İzin yok: {neden}"),
            Hata::Iptal => write!(f, "İptal edildi."),
        }
    }
}

impl std::error::Error for Hata {}
