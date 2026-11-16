//! "Aksesuar Özellikleri": the plugin's own help, in Turkish.

use serde_json::{Value, json};

pub const SATIRLAR: &[&str] = &[
    "AKSESUAR EKLENTİSİ — komutlar (Model sekmesi > Eklentiler grubu ve Eklentiler menüsü)",
    "",
    "1) Kanala Yerleştir",
    "   Ne yapar: Aksesuarı kanat kanalına oturtur ve kenar boyunca istenen mesafeye koyar.",
    "   Nasıl: Önce aksesuarın oturma yüzünü, sonra Ctrl ile kanat profilinin düz kenarını seçin.",
    "   İsterseniz kanal tabanı yüzünü de seçin (seçmezseniz kenarın yanındaki büyük düz yüz alınır).",
    "   Mesafe: sayı (mm) ya da ifade, ör. 250, KOL, FFH/2, FFH/2 - 15 mm.",
    "   'Kenarın sonundan ölç' mesafeyi kenarın öbür ucundan ölçer; '180° çevir' parçayı ters çevirir.",
    "   Sonuç: bir Düzlemsel ilişki (Kanal: <gövde>). Parametre değişince parça kendiliğinden kayar.",
    "   Aksesuar tasarımın kendi gövdesiyse önce kendi bileşenine alınır.",
    "",
    "2) Tij ve Malzeme Listesi",
    "   Ne yapar: Adı TIJ ile başlayan her gövdenin kesim boyunu (en uzun kenar, 0,5 mm'ye yuvarlı) ve",
    "   diğer parçaları koda göre (adın ilk boşluğa kadarki kısmı) sayar; parametreleri de ekler.",
    "   Çıktı: izin verdiğiniz 'cikti' klasörüne Excel'in açtığı CSV (UTF-8, ; ayraçlı, virgüllü ondalık).",
    "   Dosya adı: varsayılan tij_listesi.csv.",
    "",
    "3) Çift Açılım Şablonu",
    "   Ne yapar: Boş tasarımda FFH, FFB, KOL ölçülü çift açılım kanadını aksesuarlarıyla kurar:",
    "   kanat profili, cam, köşe aktarmalar, ispanyolet ve kol, makas, alt menteşe, tijler.",
    "   Parametreler: FFH, FFB, KOL, KA=120, IS=300, AM=150, MKR=180, PAY=1, MK (FFB'ye göre 220/420/650),",
    "   TIJ_SOL_ALT, TIJ_SOL_UST, TIJ_UST, TIJ_ALT (kesim boyları) ve CK (çocuk kilidi 1/0).",
    "   Kurallar: FFH ≤ 1200 iken 'EK KILIT 1' bastırılır; CK = 0 iken 'COCUK KILIDI' bastırılır.",
    "   Global Değişkenler panelinde FFH ya da CK'yi değiştirmeniz yeter; kilitler kendiliğinden gelip gider.",
    "   Kuralları Tasarım Tablosu > 'Kural ekle veya düzenle' bölümünde (Konfigürasyon kuralları) görebilir, değiştirebilirsiniz.",
    "",
    "4) Aksesuar Özellikleri",
    "   Bu yardım metni.",
    "",
    "Kurulum: Tercihler > Eklentiler > Eklenti yükle… ile aksesuar zip dosyasını seçin;",
    "'cikti' klasörü için tam bir klasör yolu girin (ör. C:\\Users\\<kullanıcı>\\Desktop).",
];

pub fn sonuc() -> Value {
    json!(SATIRLAR)
}
