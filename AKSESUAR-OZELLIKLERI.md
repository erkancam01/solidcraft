# Aksesuar özellikleri (Erkan'ın SolidCraft forku)

Bu dosya, pencere aksesuarı işi için bu forka eklenen her şeyi tek yerde toplar: ne olduğu,
nerede olduğu, nasıl kullanıldığı ve nereden indirileceği.

| # | Özellik | Tür | Dal | Durum |
|---|---|---|---|---|
| 1 | Türkçe arayüz (SolidWorks terimleriyle) | Çeviri | `turkce` | Hazır |
| 2 | İfadelerde `if()`, karşılaştırmalar, `and`/`or`/`not` (A1) | Motor | `a1-ifade` | Hazır |
| 3 | Çoğaltma adetlerinin parametreyle sürülmesi (A3) | Motor | `a3-cogaltma` | Geliştiriliyor |
| 4 | Aksesuar eklentisi (4 komut) | Eklenti | `aksesuar-eklentisi` | 0.1.1 uygulamada denendi (Kanala Yerleştir hariç) |
| 5 | Koşullu bastırma: konfigürasyon kuralları | SolidCraft'ta yerleşik | — | Hazır |

Eski A2 (bastırma) ve A4 (kanal) dalları bırakıldı; işleri eklentiye ve yerleşik kurallara taşındı.

## 1. Türkçe arayüz

`assets/locales/tr.json` eksik çevirilerle tamamlandı; terimler SolidWorks Türkçesine göre
(ör. Parameters = **Global Değişkenler**, Joint = **İlişki**, Pattern = **Çoğaltma**,
Suppress = **Bastır**, Configuration = **Konfigürasyon**).

- Dili seçmek: **Tercihler > Dil > Türkçe**.
- Windows taşınabilir sürüm: GitHub'da **Actions > Turkce Windows > Run workflow** (ya da `turkce`
  dalına push). Bitince çalışmanın sayfasında **Artifacts > solidcraft-turkce-windows-portable**.

## 2. İfadeler: `if()` ve karşılaştırmalar (A1, `a1-ifade`)

Global değişken ifadelerinde artık:

- Karşılaştırmalar: `<`, `<=`, `>`, `>=`, `==`, `!=` (ayrıca `≤ ≥ ≠`); sonuç 1 ya da 0.
- `if(koşul, a, b)`, `and(a, b, …)`, `or(a, b, …)`, `not(a)`.
- Birimsiz sayı karşı tarafın birimini alır: `FFH > 1200` → 1200 mm ile karşılaştırır.
- Seçilmeyen dal hesaplanmaz: `if(n > 0, L / n, 0)` n = 0 iken de çalışır.

Örnek (makas boyu):

```text
MK = if(FFB <= 650, 220 mm, if(FFB <= 1000, 420 mm, 650 mm))
```

Bu bir motor değişikliğidir: kullanmak için `a1-ifade` dalından derlenmiş SolidCraft gerekir.
Aksesuar eklentisi buna ihtiyaç duymaz (şablon `min`/`max`/`ceil` kullanır).

## 3. Çoğaltma adetleri (A3, `a3-cogaltma`)

Çoğaltma (desen) adetlerinin global değişken ya da ifadeyle sürülmesi; örneğin kilit
karşılıklarının sayısını FFH'ye bağlamak. **Henüz geliştiriliyor**, hazır değil.

## 4. Aksesuar eklentisi (`aksesuar-eklentisi`)

Kaynak: `aksesuar-eklentisi/` (bağımsız Rust projesi, WebAssembly bileşeni). Kurulduktan sonra
komutlar iki yerde görünür:

- Üst çubuktaki **Eklentiler** menüsü > **Aksesuar** alt menüsü,
- **Model** sekmesindeki **Eklentiler** grubu (her komut kendi adıyla bir düğme).

Bir komuta tıklamak girdi penceresini açar; **Çalıştır** ile çalışır. Düzenleyen komutlar tek bir
geri alma (Ctrl+Z) adımıdır.

### 4.1 Kanala Yerleştir

Aksesuarı kanat kanalına oturtur ve kenar boyunca istenen mesafeye koyar.

1. Aksesuarın **oturma yüzünü** seçin (kanala değecek düz yüz).
2. Ctrl ile kanat profilinin **düz kenarını** seçin (mesafe bu kenarın başından ölçülür).
3. İsterseniz Ctrl ile **kanal tabanı yüzünü** de seçin. Seçmezseniz kenarın yanındaki büyük düz
   yüz alınır.
4. **Kanala Yerleştir**'e tıklayın:
   - **Mesafe**: sayı (mm) ya da ifade: `250`, `250,5`, `KOL`, `FFH/2`, `FFH/2 - 15 mm`.
     Bilinmeyen bir global değişken adı yazarsanız hangisi olduğu söylenir.
   - **Kenarın sonundan ölç**: mesafeyi kenarın öbür ucundan ölçer.
   - **180° çevir**: parçayı oturma yüzü üzerinde ters çevirir.

Sonuç: aksesuarla kanat arasında bir **Düzlemsel ilişki** (`Kanal: <gövde adı>`). Oturma yüzü
tabana yapışır; parçanın uzun kenarı kenar boyunca durur; kanal genişliğinde tabanın ortasına
gelir; yan kayma ve dönüş sınırlarla kilitlidir. Mesafe ifadesi ilişkide saklanır: `KOL`
değişince parça kendiliğinden kayar. Kenar ve köşe adla izlenir; kanat boyu değişse de parça
köşeye göre aynı mesafede kalır. Aksesuar tasarımın kendi gövdesiyse önce kendi bileşenine
alınır (Gövdelerden Bileşen Oluştur).

### 4.2 Tij ve Malzeme Listesi

Tasarımı okur, hiçbir şeyi değiştirmez; izin verilen **cikti** klasörüne CSV yazar.

- **TİJLER**: adı `TIJ` ile başlayan her gövde (büyük/küçük harf fark etmez, `Tij`, `tıj`
  de olur). Kesim boyu = gövdenin en uzun kutu kenarı, 0,5 mm'ye yuvarlanmış. Sonda toplam.
- **PARÇALAR**: diğer gövdeler, adın ilk boşluğa kadarki kısmına (kod) göre gruplu, adetli
  (`KA-02 dikey` ve `KA-02 yatay` → `KA-02`, 2 adet).
- **PARAMETRELER**: global değişkenler (ad, değer, birim, ifade).
- Dosya: varsayılan `tij_listesi.csv`; **Dosya adı** alanından değiştirilir (yalnızca ad, klasör
  yazılmaz). UTF-8 (BOM'lu), `;` ayraçlı, virgüllü ondalık: Türkçe Excel'de çift tıkla açılır.
- Pencerede kısa bir özet görünür (tij sayısı, toplam boy, kodlar, dosya yeri).

### 4.3 Çift Açılım Şablonu

**Boş bir tasarımda** (Dosya > Yeni) çalıştırın. Girdiler: **FFH** (kanat yüksekliği, varsayılan
1200), **FFB** (kanat genişliği, 900), **KOL** (kol yüksekliği, 600), **Çocuk kilidi** (işaret).

Kurduğu global değişkenler: `FFH`, `FFB`, `KOL`, `KA = 120 mm`, `IS = 300 mm`, `AM = 150 mm`,
`MKR = 180 mm`, `PAY = 1 mm`, `MK` (makas boyu: FFB ≤ 650 → 220, ≤ 1000 → 420, üstü → 650),
`TIJ_SOL_ALT`, `TIJ_SOL_UST`, `TIJ_UST`, `TIJ_ALT` (tij kesim boyları), `EK = 80 mm` (ek kilit
boyu), `TIJ_SOL_ALT_1`, `TIJ_SOL_ALT_2` (ek kilit gelince sol alt tijin iki parçası) ve `CK`
(çocuk kilidi: 1 var, 0 yok).

Model **dik** kurulur (SolidCraft'ta yukarı yön Z): kanat XZ düzleminde durur, oda tarafı (kol)
-Y yönüne bakar. Sürüm 0.1.1'den önce model yatık kuruluyordu.

Kurduğu parçalar: kanat profili (4), cam, köşe aktarmalar KA-01/KA-02, ispanyolet IS-300, kol
rozeti ve tutamağı, makas, alt menteşe, 4 tij; ayrıca **EK KILIT 1** (kol tarafında, alt köşe
aktarma ile ispanyoletin altı arasındaki bölgenin tam ortasında), bu kilidin iki yanındaki
**TIJ sol alt 1** / **TIJ sol alt 2** ve **COCUK KILIDI** (kol rozetinin üstünde).

FFH 1200 mm'nin üstünde ek kilit gelir ve sol alt tij ikiye bölünür: `TIJ sol alt` bastırılır,
yerine iki parça gelir. Her parça en az 40 mm olmalı; bu yüzden ek kilitli ölçülerde KOL en az
434 mm olmalı (şablon küçük değeri Türkçe uyarıyla reddeder). Tij listesi (4.2) bastırılmış
gövdeleri listelemez, yani her ölçüde kesilecek tijler doğru çıkar.

Konfigürasyon kuralları (aşağıdaki 5. bölüm):

| Kural | Koşul | Etki |
|---|---|---|
| Ek kilit / sol alt tij parçaları: FFH 1200 mm ve altında yok | FFH ≤ 1200 mm | EK KILIT 1, TIJ sol alt 1 ve 2 bastırılır; TIJ sol alt gelir |
| Ek kilit / sol alt tij parçaları: FFH 1200 mm üstünde var | FFH > 1200 mm | EK KILIT 1, TIJ sol alt 1 ve 2 gelir; TIJ sol alt bastırılır |
| Çocuk kilidi: CK = 0 ise yok | CK = 0 | COCUK KILIDI bastırılır |
| Çocuk kilidi: CK = 1 ise var | CK ≠ 0 | COCUK KILIDI geri gelir |

Sonra **Global Değişkenler** panelinde FFH, FFB, KOL ya da CK'yi değiştirmeniz yeter: ölçüler
güncellenir, kilitler kendiliğinden gelir/gider. Tasarımda bu adlarda bir global değişken varsa
şablon hiçbir şey yapmadan durur ve nedenini söyler.

### 4.4 Aksesuar Özellikleri

Bu komutların Türkçe yardım metnini pencerede gösterir.

## 5. Koşullu bastırma: konfigürasyon kuralları (yerleşik)

SolidCraft'ın kendi özelliğidir; eklenti olmadan da kullanılır. Bir parametreye göre bir
özelliği (gövdeyi) bastırmak için:

1. **Tasarım Tablosunu Göster** (Konfigürasyon) ile tabloyu açın; **Sütun ekle** ile bastırılacak
   özelliği (Bastır sütunu) ekleyin. Tablo boşsa ilk satır ("Default") kendiliğinden oluşur.
2. Tablonun altında **Kural ekle veya düzenle**: kurala ad verin, koşulu seçin (ör. `FFH` `≤`
   `1200 mm`), etkiyi **bastırmayı sür** yapın ve özelliği seçin. Geri getirmek için tersi koşulla
   (`FFH > 1200 mm`, bastırma: hayır) ikinci bir kural ekleyin.
3. Kurallar sırayla çalışır. **Tümü / Herhangi / Değil** ile bileşik koşul kurulabilir.
4. Bir kez satırı **Etkinleştir**in; sonra global değişkeni her değiştirdiğinizde, o değişkeni
   okuyan bastırma kuralları kendiliğinden çalışır.

Aynı şey komutla (ajan ya da CLI):

```json
{"command":"config.configure","params":{"params":["FFH"],"features":["EK KILIT 1"]}}
{"command":"config.rule","params":{"rules":[
  {"id":"ek-kilit-kapali","name":"Ek kilit yok","condition":{"type":"parameter","parameter":"FFH","operator":"le","expression":"1200 mm"},"effect":{"type":"drive_suppression","feature":ÖZELLİK_NO,"value":true}},
  {"id":"ek-kilit-acik","name":"Ek kilit var","condition":{"type":"parameter","parameter":"FFH","operator":"gt","expression":"1200 mm"},"effect":{"type":"drive_suppression","feature":ÖZELLİK_NO,"value":false}}]}}
{"command":"config.activate","params":{"row":"Default"}}
```

Ayrıntı: `docs/configurations.md`, "Ordered rules and preview".

## 6. Eklentiyi kurmak

1. Eklenti zip'ini indirin (8. bölüm) ya da bilgisayarda derleyin (7. bölüm) ve indirilen `aksesuar-eklentisi.zip` artifact'ını açın:
   içindeki **`aksesuar-eklentisi.zip`** kurulacak dosyadır (içinde `plugin.json` ve
   `aksesuar.wasm` var).
2. SolidCraft'ta **Tercihler > Eklentiler > Eklenti yükle…** (ya da **Eklentiler** menüsü >
   **Eklenti yükle…**) ve bu zip'i seçin.
3. Onay ekranında **cikti** klasörü için tam bir klasör yolu girin (ör.
   `C:\Users\<kullanıcı>\Desktop\Tij`); tij listesi oraya yazılır. **Yükle**'ye basın.
4. **Tercihler > Eklentiler**'de Aksesuar'ın **Etkin** olduğundan emin olun.

Güncellerken aynı yolla yeni zip'i yükleyin (eskisinin yerine geçer; klasör iznini yeniden verin).
Kaldırmak: Tercihler > Eklentiler > Aksesuar > **Kaldır**. Eklenti yalnızca masaüstü sürümde
çalışır (web sürümünde çalışmaz).

## 7. Bilgisayarda derleme ve deneme

Kaynak `C:\Users\PC\Desktop\aksesuar\solidcraft` klasöründe (forkun klonu). Gereken araçlar
kurulu: Git, Rust 1.99.1 (`wasm32-unknown-unknown` hedefiyle), Visual Studio 18 (MSVC),
`wasm-tools`, Node.

Eklentiyi derlemek (`aksesuar-eklentisi` dalında, `aksesuar-eklentisi/` klasöründe):

```text
cargo test --locked
cargo build --locked --release --target wasm32-unknown-unknown
wasm-tools component new target/wasm32-unknown-unknown/release/aksesuar.wasm -o dist/aksesuar/aksesuar.wasm
copy plugin.json dist\aksesuar\
```

`dist\aksesuar` içindeki `plugin.json` ve `aksesuar.wasm` zip'lenip kurulur.

Uygulamasız deneme: `solidcraft-cli.exe` eklentiyi doğrudan çalıştırabilir (`plugin.inspect` ile
onay özeti alınır, `plugin.run` ile çalışır, `snapshot` ile görüntü alınır). Dosya yazan komut
(Tij listesi) CLI'da izin alamaz; onu uygulamada ya da uygulamanın kontrol kanalından çalıştırın.

### 0.1.1 deneme sonucu (16.11.2026)

Türkçe SolidCraft 0.43 üzerinde, FFH 1500 / FFB 900 / KOL 600:

- Şablon hatasız kuruldu; model **dik** duruyor (önden ve izometrik görüntüyle bakıldı).
- Ek kilit var; sol alt tij **123 + 123 mm** iki parçaya bölündü, tek parça bastırıldı.
- CSV doğru: 5 tij, toplam 1860 mm; bastırılan `TIJ sol alt` listede yok.
- FFH 1100'e düşürülünce kurallar geri döndü: ek kilit ve iki parça bastırıldı, `TIJ sol alt`
  (328 mm) geldi; CSV 4 tij, toplam 1542 mm.

## 8. İndirme yerleri

GitHub: `erkancam01/solidcraft` > **Actions**.

| Ne | İş akışı (workflow) | Artifact adı |
|---|---|---|
| Aksesuar eklentisi | **Aksesuar eklentisi** (`aksesuar-eklentisi` dalına push ya da Run workflow) | `aksesuar-eklentisi` |
| Türkçe Windows SolidCraft | **Turkce Windows** | `solidcraft-turkce-windows-portable` |

Çalışmanın sayfasının en altında **Artifacts** bölümünden indirilir (GitHub'a giriş gerekir;
30 gün saklanır).

## 9. Bilinen sınırlar

- Kanala Yerleştir gerçek bir STEP parçasıyla henüz denenmedi. İlk kullanımda sonucu gözle
  kontrol edin; parça ters ya da yanlış eksende durursa **180° çevir**'i deneyin ve bildirin.
- Kanala Yerleştir, kanal tabanının köşesinde iki kenarın dik kesiştiğini (dikdörtgen taban)
  varsayar. Yan konum (kanal ortası) yerleştirme anındaki taban genişliğinden alınır.
- Tij listesi bileşen yerleşimlerini ayrı saymaz: her gövde bir kez sayılır (bileşen listesi
  okuma-yalnız eklentilere açık değil).
- Seçim, girdi penceresinde çizilemediği için komut **o anki seçimi** okur: önce seçin, sonra
  komuta tıklayın.
