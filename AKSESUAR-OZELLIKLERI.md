# Aksesuar özellikleri (Erkan'ın SolidCraft forku)

Bu dosya, pencere aksesuarı işi için bu forka eklenen her şeyi tek yerde toplar: ne olduğu,
nerede olduğu, nasıl kullanıldığı ve nereden indirileceği.

| # | Özellik | Tür | Dal | Durum |
|---|---|---|---|---|
| 1 | Türkçe arayüz (SolidWorks terimleriyle) | Çeviri | `turkce` | Hazır |
| 2 | İfadelerde `if()`, karşılaştırmalar, `and`/`or`/`not` (A1) | Motor | `a1-ifade` | Hazır |
| 3 | Çoğaltma adetlerinin parametreyle sürülmesi (A3) | Motor | `a3-cogaltma` | Geliştiriliyor |
| 4 | Aksesuar eklentisi (6 komut) | Eklenti | `aksesuar-yerlesim` (0.1.2'ye kadar `aksesuar-eklentisi`) | 0.1.5; altı komut da uygulamada denendi (0.1.4) |
| 5 | Koşullu bastırma: konfigürasyon kuralları | SolidCraft'ta yerleşik | — | Hazır |
| 6 | Kolay montaj (SolidWorks gibi) | Motor + arayüz | `montaj-kolay` | Uygulamada denendi |
| 7 | Aksesuar Kütüphanesi + yerleşim noktaları + tıkla-yerleştir | Motor + arayüz | `aksesuar-yerlesim` | Hazır — uygulamada denendi |

**`aksesuar-yerlesim` dalı hepsini içerir:** `turkce`, `a1-ifade`, `aksesuar-eklentisi` ve
`montaj-kolay` bu dala birleştirildi (merge). Yalnızca `a3-cogaltma` ayrı duruyor. Derlemek ve
denemek için bu dalı kullanın.

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

Bu bir motor değişikliğidir: kullanmak için `a1-ifade` (ya da hepsini içeren `aksesuar-yerlesim`)
dalından derlenmiş SolidCraft gerekir. Aksesuar eklentisi buna ihtiyaç duymaz (şablon
`min`/`max`/`ceil` kullanır). Yerleşim noktalarının koşulları (`FFH > 1200 mm`, 7.3) bu
karşılaştırmalarla yazılır.

## 3. Çoğaltma adetleri (A3, `a3-cogaltma`)

Çoğaltma (desen) adetlerinin global değişken ya da ifadeyle sürülmesi; örneğin kilit
karşılıklarının sayısını FFH'ye bağlamak. **Henüz geliştiriliyor**, hazır değil.

## 4. Aksesuar eklentisi (`aksesuar-eklentisi`)

Kaynak: `aksesuar-eklentisi/` (bağımsız Rust projesi, WebAssembly bileşeni). Sürüm **0.1.5**,
6 komut. En yeni hâli `aksesuar-yerlesim` dalında; `aksesuar-eklentisi` dalı 0.1.2'de kaldı.
0.1.3 ve sonrası SolidCraft'ın yeni yerleşim noktası komutlarını (`hardware.slot.*`) kullanır:
`aksesuar-yerlesim` dalından derlenmiş SolidCraft ile kullanın.

Kurulduktan sonra komutlar iki yerde görünür:

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

**STEP parçaları** (0.1.2): **Ekle > STEP** ile eklenen parça, gövde başına bir alt bileşenden
oluşan bir bileşen olarak gelir ve sabitlenmiş (grounded) durur. Kanala Yerleştir parçanın **tüm
gövdelerini birlikte** taşır (en dıştaki bileşeni) ve sabitlemesini kendisi kaldırır; pencerede
bunu söyler. Oturma yüzü parçanın herhangi bir gövdesinden seçilebilir.

**Mesafe oturma yüzünün ortasına** kadardır. Köşe aktarma gibi köşeye dayanan parçalarda köşeden
yüz ortasına olan payı ekleyin. Örnek: YMB.00146 tırnaklı köşe aktarmada dikey kolun iç yüzü
oturma yüzüdür ve pay **30,9 mm**'dir (**Kenarın sonundan ölç** + **180° çevir**, mesafe `30,9`).

Parçanın yönü oturma yüzünün **kendi düzlemindeki** en uzun düz kenardan alınır (0.1.1'de
gövdenin başka yüzlerindeki kenarlar da sayılıyordu; karmaşık parçalarda eğik oturabiliyordu).

Kütüphanedeki parçalar için daha kolay yol: **tıkla-yerleştir** (7.4). Kanala Yerleştir, kütüphanede
olmayan ya da yerleşim noktası olmayan parçalar için kalır.

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

Sonra **Global Değişkenler** panelinde FFH, FFB, KOL ya da CK'yi değiştirmeniz yeter (ya da
**Ölçüleri Değiştir**, 4.5): ölçüler güncellenir, kilitler kendiliğinden gelir/gider. Tasarımda bu
adlarda bir global değişken varsa şablon hiçbir şey yapmadan durur ve nedenini söyler.

**Yerleşim noktaları** (0.1.3): şablon son adımda kanada 9 yerleşim noktası yazar (4 köşe, alt ve
üst menteşe, makas, kol, FFH > 1200'de ek kilit). Noktalar Aksesuar Kütüphanesi panelinde görünür
ve ölçülerle birlikte kayar; ayrıntı 7.3'te.

### 4.4 Koşullu Parçaları Güncelle (0.1.4)

Koşullu yerleşim noktalarına (ör. Ek kilit 1: FFH > 1200 mm) yerleştirilmiş parçaları o koşula
bağlar. Ayrıntı 7.5'te.

### 4.5 Ölçüleri Değiştir (0.1.4)

FFH, FFB, KOL ve çocuk kilidini tek pencerede, tek adımda değiştirir. Ayrıntı 7.6'da.

### 4.6 Aksesuar Özellikleri

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

## 6. Kolay montaj (`montaj-kolay`)

SolidWorks'teki montaj alışkanlığı için yapılan program (motor + arayüz) değişiklikleri. Eklentiyle
yapılamadı: eklentiler ekrana menü çizemiyor ve seçimi anlık izleyemiyor.

### 6.1 Eklenen parça hareketli gelir

**STEP Ekle** ve **Tasarım Ekle** ile gelen parça artık hareketlidir. Sabit gelen tek parça, boş bir
tasarıma eklenen **ilk parçadır** (SolidWorks'teki gibi). Tasarımın kendi gövdeleri varsa (ör. Çift
Açılım Şablonu'nun kanadı) onlar zaten sabittir; eklenen parça hareketli gelir. Sağ tık ya da
"bileşene çevir" gerekmez. Çok gövdeli STEP parçası tek parça gibi birlikte hareket eder.

### 6.2 Hızlı ilişki kartı

Komut açık değilken **iki farklı parçadan** birer yüz, kenar ya da delik seç (Ctrl ile ikinciyi
ekle). Seçtiğin yerin yanında küçük bir kart açılır ve yalnızca uyan ilişkileri gösterir:

| Seçim | Kartta |
|---|---|
| İki düz yüz | Çakışık, Paralel, Dik, Mesafe, Açı |
| İki delik / silindir | Eş Merkezli (+ Paralel, Dik, Açı) |
| Silindir + düz yüz | Teğet, Mesafe, Paralel, Dik, Açı |

- Birine tıkla: ilişki hemen kurulur, parça yerine gider (tek Ctrl+Z ile geri alınır).
- **Mesafe** ve **Açı** önce değeri sorar (`25 mm`, `KOL/2`, `90 deg` gibi), sonra **Uygula**.
- Kurulduktan sonra kart **✓ Çakışık · Ters çevir** olur: **Ters çevir** parçayı yüzün öbür
  yönüne döndürür (SolidWorks'teki hizalı / ters hizalı).
- Sabit olan taraf hep yerinde kalır; hangi sırayla seçtiğin fark etmez.
- Dişli, kam gibi ilişkiler kartta yok; onlar **Montaj > Montaj İlişkisi** penceresinde.

### 6.3 SolidWorks isimleri

Montaj ilişkilerinin adları: **Çakışık**, **Çakışık (hizalı)**, **Eş Merkezli**, **Paralel**,
**Dik**, **Teğet**, **Mesafe**, **Açı**. Komutun adı **Montaj İlişkisi** (eski: Kısıtla).

### 6.4 Parçayı fareyle sürükleme

Programda zaten vardı: komut açık değilken parçayı tutup sürükle. Serbest parça serbestçe taşınır;
ilişkili parça yalnızca serbest kaldığı yönlerde kayar ya da döner; sabit ya da tam tanımlı parça
oynamaz. Bu sürümde eklenen: ilişkili parçayı sürüklerken **Esc** parçayı geri koyar ve sürükleme
tek bir geri alma adımı olarak kalır.

### 6.5 Deneme sonucu (16.11.2026)

Çift Açılım şablonuna (FFH 1500) köşe aktarma STEP'i eklendi: parça hareketli geldi. Kanat sağ'ın
dış yüzü ile parçanın oturma yüzü seçilince kart açıldı (Çakışık, Paralel, Dik, Mesafe, Açı);
Çakışık'a tıklayınca dört gövde birlikte x = 900'deki yüze oturdu; Ters çevir parçayı döndürdü ve
her tıklamada değişti.

## 7. Aksesuar Kütüphanesi ve yerleşim (`aksesuar-yerlesim`)

Aksesuarı kütüphaneden seçip kanattaki hazır bir noktaya tek tıkla oturtmak için yapılan program
(motor + arayüz) değişiklikleri. Eklentiyle yapılamadı: noktaları görünümde çizmek, önizleme ve
görünümde tıklama eklentiye açık değil.

İş sırası:

1. Kütüphane klasörü ve panel (7.1).
2. Her parça için bir kez **bağlantı noktası** (7.2): parçanın neresiyle oturacağı.
3. Kanatta **yerleşim noktaları** (7.3): parçanın nereye geleceği. Çift Açılım Şablonu yazar.
4. **Tıkla-yerleştir** (7.4).
5. Koşullu noktalardaki parçalar için **Koşullu Parçaları Güncelle** (7.5).

### 7.1 Kütüphane klasörü ve panel

Klasör: `C:\Users\PC\Desktop\aksesuar\kutuphane\<Grup>\`. Her alt klasör bir gruptur:

| Grup klasörü | Şablondaki yerleşim noktaları (7.3) |
|---|---|
| Köşe aktarma | Sol alt, Sol üst, Sağ üst, Sağ alt köşe |
| Menteşe grubu | Alt menteşe, Üst menteşe |
| Makas ve kol | Makas, Kol |
| Kilit ispanyolet tij | Ek kilit 1 |
| Profil | — (kanat profilleri; kanat oluşturma sonraki aşamada) |

- Listelenen dosyalar: `.step`, `.stp` ve `.dxf` (büyük/küçük harf fark etmez). **Profil**
  grubunda ayrıca SolidCraft tasarımları (`.sc` ve kart olmayan `.json`).
- **Parça kartı**: dosyanın yanında aynı adlı `<ad>.json`. Alanları: `part_no` (parça no),
  `description` (açıklama), `role` (`hardware` aksesuar / `profile` profil), `connection`
  (bağlantı noktası, 7.2). Kart yoksa parça yine listelenir. `role` yoksa Profil klasöründekiler
  profil, diğerleri aksesuar sayılır.
- En çok 64 grup ve 2000 parça listelenir.

Panel: **Montaj > Bileşenler > Aksesuarlar** sağda **Aksesuar Kütüphanesi** panelini açar.

- **Klasör** alanı, **Klasör seç…** ve **Yenile**. Klasör seç'te kütüphane klasöründeki ya da bir
  grup klasöründeki herhangi bir STEP, DXF veya tasarım dosyasını seçin. Klasör tercih olarak
  saklanır; bir kez seçmek yeter.
- **Ara**: ada, parça numarasına ve açıklamaya göre süzer.
- Her satırda küçük resim, ad, parça no ve **Bağlantı noktası tanımlı** / **Bağlantı noktası yok**.
- **Tasarıma ekle** (ya da çift tık): STEP parçasını yerleşim noktası kullanmadan tasarıma ekler.
  Profil ve DXF için kanat oluşturma sonraki aşamada.
- Bağlantı noktası tanımlı bir aksesuara tıklamak yerleştirme modunu açar (7.4).

### 7.2 Bağlantı noktası

Parçanın kanala hangi yüzüyle ve hangi köşesinden oturacağı. Her parça için bir kez tanımlanır,
parça kartına yazılır.

1. Kütüphanede parçayı seçin, **Bağlantı noktası…**'na tıklayın (tanımlıysa **Bağlantı noktasını
   değiştir…**). Parça tasarıma bir anlığına gelir; görünümde küçük bir kart açılır.
2. **Oturma yüzü**: parçanın kanala oturan düz yüzüne tıklayın.
3. **Referans kenar**: bu yüzde, ölçünün alındığı köşeden başlayan düz kenara tıklayın. Tıkladığınız
   yere yakın uç başlangıç olur. **Kenarın ortasından** işaretliyse kenarın ortası alınır.
4. **Yanalda yüzün ortası** (varsayılan açık): başlangıç, oturma yüzünün genişliğinde ortaya
   kayar. Böylece parça kanalın ortasına oturur.
5. **Kaydet**: bağlantı noktası parçanın kendi (STEP) koordinatında karta yazılır. Parça
   tasarımdan kalkar; **tasarım değişmez**, geri alma adımı kalmaz. İptal, Esc ve kartı kapatmak
   da tasarımı eski hâline döndürür.

**İpucu:** kenarın, parçanın referans köşesine yakın ucuna tıklayın (köşe aktarmada kanat
köşesine gelen uç).

**Dikkat:** köşe aktarmanın şimdiki kartı (`Köşe aktarma\YMB.00146 - TIRNAKLI KÖŞE AKTARMA.json`)
denemede seçilmiş **deneme verisidir**. Gerçek kullanımdan önce bağlantı noktasını yeniden
tanımlayın.

### 7.3 Yerleşim noktaları

Yerleşim noktası: kanatta bir kütüphane grubunun parçasının geleceği adlandırılmış yer. Tasarımda
saklanır (tasarımla kaydedilir) ve FFH, FFB, KOL değişince kendiliğinden kayar: kenar ve yüz adla
yeniden bulunur.

| Alan | Anlamı |
|---|---|
| **Ad** | ör. Sol alt köşe |
| **Grup** | hangi kütüphane grubunun parçası gelir (ör. Köşe aktarma) |
| **Taban yüzü** + **Kenar** | profilin aksesuar yüzü ve bu yüzün düz kenarı; kenarda tıklanan yere yakın uç köşedir |
| **Kenar boyunca** | köşeden mesafe: sayı ya da ifade (`0`, `KOL`, `(KA + KOL - IS/2)/2`) |
| **Yanal** | **Ortala** (yüzün ortası) ya da mesafe / ifade |
| **Açı** | taban yüzü üzerinde dönüş |
| **Ters** | parçayı ters çevirir |
| **Koşul** | boşsa nokta hep kullanılır; ör. `FFH > 1200 mm`. Koşul tutmuyorsa: **Etkin değil (koşul)** |

Panel: Aksesuar Kütüphanesi'nin altında **Yerleşim noktaları** bölümü. Noktalar gruplara göre
listelenir.

- **Nokta ekle…**: taban yüzüne ve kenara tıklayın, kartta alanları doldurun. Noktanın yeri
  görünümde eksen üçlüsüyle gösterilir. **Kaydet** tek geri alma adımıdır; İptal ve Esc hiçbir şey
  değiştirmez.
- **Noktayı düzenle**: aynı kart. Yüz ve kenarı yeniden seçmezseniz noktanın yeri değişmez.
  Noktada parça varsa parça da aynı adımda yeni yere taşınır.
- **Noktayı sil**: noktada parça varsa parça da kaldırılır (önce onay sorulur).
- Kütüphane açıkken her nokta 3B görünümde bir işaretle çizilir (koşulu tutmuyorsa soluk); üstüne
  gelince adı görünür.

Çift Açılım Şablonu (eklenti 0.1.3 ve sonrası) şu 9 noktayı yazar (önden bakış: kol tarafı solda
x = 0, menteşe tarafı sağda x = FFB, oda tarafı y = 0):

| Nokta | Grup | Yer | Kenar boyunca | Koşul |
|---|---|---|---|---|
| Sol alt köşe | Köşe aktarma | sol profilin dış yüzü, alt köşe | `0` | — |
| Sol üst köşe | Köşe aktarma | sol profilin dış yüzü, üst köşe | `0` | — |
| Sağ üst köşe | Köşe aktarma | sağ profilin dış yüzü, üst köşe | `0` | — |
| Sağ alt köşe | Köşe aktarma | sağ profilin dış yüzü, alt köşe | `0` | — |
| Alt menteşe | Menteşe grubu | sağ profilin alt alın yüzü, sağ alt köşe | `0` | — |
| Üst menteşe | Menteşe grubu | sağ profilin üst alın yüzü, sağ üst köşe | `0` | — |
| Makas | Makas ve kol | üst profilin dış yüzü, menteşe tarafından | `0` | — |
| Kol | Makas ve kol | sol profilin **oda tarafındaki** yüzü (rozetin olduğu yüz), profilin ortası | `KOL` | — |
| Ek kilit 1 | Kilit ispanyolet tij | sol profilin dış yüzü, şablonun ek kilidi ortaladığı yer | `(KA + KOL - IS/2)/2` | `FFH > 1200 mm` |

Kol noktasının yeri kararlaştırıldı: oda tarafındaki yüzde, KOL yüksekliğinde.

### 7.4 Tıkla-yerleştir

1. Aksesuar Kütüphanesi'nde bağlantı noktası tanımlı bir aksesuara tıklayın. Üstte
   "Yerleştirme: <parça> — uygun noktaya tıklayın (Esc: bitir)" yazar. Bağlantı noktası yoksa
   "Önce bağlantı noktasını tanımlayın" der.
2. Parçanın grubuna uyan, etkin ve boş noktalar görünümde parlar.
3. Fareyle bir noktanın üstüne gelin: parça orada **yarı saydam önizleme** olarak görünür.
4. Tıklayın: parça yerleşir. Tek Ctrl+Z ile geri alınır. Dolu nokta küçük gri, onaylı bir işaret
   olur.
5. Mod açık kalır: aynı parçayı başka noktalara da koyun (ör. dört köşe aktarma). Aynı dosya
   ikinci kez **kopyası** olarak gelir: tek bileşen, dosya bir kez okunur.
6. Yerleştirmeden sonra parçanın yanında küçük bir kart: **Ters çevir** (yüzün öbür yönü),
   **180° çevir** (taban yüzü üzerinde yarım tur), **Kaldır** (yerleşimi geri alır).
7. Bitirmek: **Esc**, görünümde sağ tık ya da aynı parça satırına yeniden tıklama.

Sonuç: her parça için bir **Düzlemsel ilişki** (`<nokta adı>: <parça adı>`). Parça adı kartın
`part_no`'su, yoksa dosya adı. Kenar boyunca ifadesi ilişkide saklanır: FFH, FFB, KOL değişince
parçalar yeni yerlerine kendiliğinden kayar.

### 7.5 Koşullu parçalar: Koşullu Parçaları Güncelle (eklenti 0.1.4)

Tıkla-yerleştir parçayı koyar ama noktanın koşuluna bağlamaz. Koşullu noktalara (ör. Ek kilit 1)
parça yerleştirdikten sonra **Koşullu Parçaları Güncelle**'yi çalıştırın (girdisi yok).

- Her koşullu noktadaki parçanın içe aktarma (Import) özelliğini Tasarım Tablosu'na sütun olarak
  ekler ve iki konfigürasyon kuralı kurar (`kosul-<nokta>-on` / `kosul-<nokta>-off`, 5. bölüm).
- Sonra FFH değişince parça **canlı** gelir/gider: koşul tutunca görünür, tutmayınca bastırılır.
  Eklentiyi yeniden çalıştırmak gerekmez. Parçanın ilişkisi bastırılmışken de bozulmaz.
- Tek karşılaştırma (`FFH > 1200 mm`) doğrudan kuralın koşulu olur. Birleşik koşul
  (`and(FFH > 1400 mm, FFB > 800 mm)`) `KOSUL_<NOKTA>` adlı birimsiz bir parametreye yazılır.
- Aynı STEP'i paylaşan yerleşimler: koşullu olan parça aynı yerde **bağımsız** bir kopyaya ayrılır,
  ilişkisi yeniden kurulur. Yoksa bastırma öbür kopyaları da götürürdü.
- Tekrar çalıştırmak güvenlidir: kurallar yinelenmez, yeni parçalar eklenir. Koşulu kaldırılan
  noktanın kuralları silinir, parçası geri gelir.
- Şablonun kendi kutuları (EK KILIT 1, COCUK KILIDI, sol alt tijler) kendi kurallarıyla kalır.
- Tek geri alma adımı.

### 7.6 Ölçüleri Değiştir (eklenti 0.1.4)

Form: **FFH**, **FFB**, **KOL** (mm) ve **Çocuk kilidi** (**Değiştirme** / **Evet** / **Hayır**).

- Boş bırakılan alan değişmez.
- Ölçüler şablonun sınırlarıyla denetlenir (tijler yeterince uzun kalmalı).
- Tek bir parametre düzenlemesidir: kurallar hemen çalışır, kilitler ve koşullu parçalar gelip
  gider. **Tek geri alma adımı.**
- Özet eski → yeni değerleri ve kullanımı değişen koşullu noktaları gösterir.
- Tasarımda FFH, FFB ve KOL yoksa önce Çift Açılım Şablonu'nu çalıştırın.

### 7.7 Komutlar (ajan ya da CLI)

| Komut | Ne yapar |
|---|---|
| `hardware.library.list {folder}` | Kütüphaneyi gruplar, parçalar ve kartlarla listeler |
| `hardware.card.set {path, part_no?, description?, role?, connection?}` | Parça kartını yazar |
| `hardware.slot.add` / `edit` / `delete` / `list` | Yerleşim noktaları; `list` noktanın dünyadaki yerini ve etkin olup olmadığını verir |
| `hardware.place {slot, path, preview?}` | Parçayı noktaya oturtur; `preview` tasarımı değiştirmeden yerini verir |
| `hardware.unplace {slot}` | Noktadaki parçayı kaldırır |

### 7.8 Deneme sonuçları (17.11.2026)

| Deneme | Sonuç |
|---|---|
| Kütüphane paneli | 7/7 |
| Bağlantı noktası belirleme | tüm adımlar geçti |
| Yerleşim noktaları | 7/7: noktalar doğru yerde; FFH, FFB ve KOL değişince izliyor |
| Tıkla-yerleştir | 8/8: dört köşeye köşe aktarma; tek bileşen, 4 kopya; FFH 1100 / FFB 1000 yapılınca izliyor |
| Eklenti 0.1.4 | Koşullu Parçaları Güncelle ve Ölçüleri Değiştir; ilişki hatası yok; her çalıştırma tek geri alma adımı |

Eklenti 0.1.4 ile FFH değiştirilince görünen kilitler:

| FFH | Ek kilit (EK KILIT 1 ve Ek kilit 1 noktasındaki parça) | Sol alt tij |
|---|---|---|
| 1300 | görünür | iki parça (TIJ sol alt 1 ve 2) |
| 1100 | bastırılmış | tek parça (TIJ sol alt) |
| 1500 | görünür | iki parça (TIJ sol alt 1 ve 2) |

Testler: motor hardware testleri **21/21**, eklenti testleri **56/56**.

### 7.9 0.1.5 / inceleme düzeltmeleri (17.11.2026)

- **Kare olmayan (gönyeli) köşeler**: yerleşim noktası seçilen kenar boyunca hesaplanır. Kare
  köşede sonuç eskisiyle aynı.
- **Dolu noktayı düzenlemek** (`hardware.slot.edit`) parçayı aynı adımda yeni yere taşır; **dolu
  noktayı silmek** (`hardware.slot.delete`) parçayı da kaldırır (arayüzde onay sorulur). Tek geri
  alma adımı ikisini birlikte geri getirir.
- Bastırılmış ya da kurallı bir bileşen yeni yerleşimde kopya olarak **yeniden kullanılmaz**.
- Parça kartının başındaki UTF-8 BOM (Not Defteri'nin eklediği işaret) artık sorun çıkarmaz.
- Bağlantı noktası ya da nokta modu açıkken belge değiştiren komutlar, kaydetme, sekme
  değiştirme/kapatma ve çıkış önce modu bitirir; otomatik kurtarma geçici parçayı yazmaz.
- Canlı sürükleme sırasında kütüphane paneli nokta listesini yeniden hesaplamaz.
- Hayalet önizleme arka planda üretilir; yerleştirme modu açıkken hızlı ilişki, döndürme ve
  sürükleme kapalı.
- **Eklenti 0.1.5**: uzun nokta adlarında kural ve parametre adları kısaltılır, özetle (hash)
  benzersiz kalır. Eklentinin kuralları **"Koşullu parça: …"** adıyla tanınır; kullanıcının kendi
  `kosul-` kuralları korunur.
- Aynı günün önceki düzeltmeleri: koşulu tutmayan noktaya da yerleştirilir (parça hemen
  bastırılır); yerleştirme kartı parçayı izler; önizleme daha belirgin.

Testler: motor hardware **26/26**, ui-egui **159/159**, eklenti **58/58**.

## 8. Eklentiyi kurmak

1. Eklenti zip'ini indirin (10. bölüm) ya da bilgisayarda derleyin (9. bölüm). İndirilen
   `aksesuar-eklentisi.zip` artifact'ını açın: içindeki **`aksesuar-eklentisi.zip`** kurulacak
   dosyadır (içinde `plugin.json` ve `aksesuar.wasm` var).
2. SolidCraft'ta **Tercihler > Eklentiler > Eklenti yükle…** (ya da **Eklentiler** menüsü >
   **Eklenti yükle…**) ve bu zip'i seçin.
3. Onay ekranında **cikti** klasörü için tam bir klasör yolu girin (ör.
   `C:\Users\<kullanıcı>\Desktop\Tij`); tij listesi oraya yazılır. **Yükle**'ye basın.
4. **Tercihler > Eklentiler**'de Aksesuar'ın **Etkin** olduğundan emin olun.

Güncellerken aynı yolla yeni zip'i yükleyin (eskisinin yerine geçer; klasör iznini yeniden verin).
Kaldırmak: Tercihler > Eklentiler > Aksesuar > **Kaldır**. Eklenti yalnızca masaüstü sürümde
çalışır (web sürümünde çalışmaz). 0.1.3 ve sonrası için SolidCraft'ın `aksesuar-yerlesim`
dalından derlenmiş olması gerekir (4. bölüm).

## 9. Bilgisayarda derleme ve deneme

Kaynak `C:\Users\PC\Desktop\aksesuar\solidcraft` klasöründe (forkun klonu). Gereken araçlar
kurulu: Git, Rust 1.99.1 (`wasm32-unknown-unknown` hedefiyle), Visual Studio 18 (MSVC),
`wasm-tools`, Node.

**Kural: önce eklenti.** Bir iş eklentiyle yapılabiliyorsa önce eklentide yapılır: eklenti
~1 dakikada derlenir. Uygulama (motor + arayüz) değişiklikleri biriktirilir ve tek derlemede
denenir.

Eklentiyi derlemek (`aksesuar-yerlesim` dalında, `aksesuar-eklentisi/` klasöründe):

```text
cargo test --locked
cargo build --locked --release --target wasm32-unknown-unknown
wasm-tools component new target/wasm32-unknown-unknown/release/aksesuar.wasm -o dist/aksesuar/aksesuar.wasm
copy plugin.json dist\aksesuar\
```

`dist\aksesuar` içindeki `plugin.json` ve `aksesuar.wasm` zip'lenip kurulur.

Uygulamayı deneme için derlemek (kök klasörde, `hizli` profili):

```text
cargo build --locked -p solidcraft --profile hizli --target x86_64-pc-windows-msvc
```

- Çıktı: `target\x86_64-pc-windows-msvc\hizli\solidcraft.exe`.
- İlk derleme ~30 dakika sürer; sonrakiler artımlıdır (yalnızca değişen kısım derlenir).
- `hizli` hafif optimize eder, LTO yapmaz. Teslim zip'leri yine `--release` ile derlenir.

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

### 0.1.2 Kanala Yerleştir deneme sonucu (16.11.2026)

Parça: `YMB.00146 - TIRNAKLI KÖŞE AKTARMA.STEP` (SolidWorks 2022, 4 gövde, L biçimli: dikey kol
~150 mm, yatay kol ~88 mm). Şablonun (FFH 1500, FFB 900) sağ üst köşesine yerleştirildi: oturma
yüzü dikey kolun iç yüzü, kenar Kanat sağ'ın dış dikey kenarı, sondan ölç, 180° çevir, mesafe 30,9.

- 0.1.1'de bulunan hata: yalnızca seçilen gövde taşındı, diğer 3 gövde yerinde kaldı. 0.1.2'de
  düzeltildi.
- 0.1.2 sonucu: dört gövde birlikte taşındı; dikey kol sağ profilin dış yüzünde (x = 900),
  yatay kolun alt yüzü tam üst profilin dış yüzünde (z = 1500). Tırnak, kanala ~3,8 mm iniyor
  (şablon profili düz kutu olduğu için içine giriyor; gerçek profilde kanala oturur).
- FFH 1300 / FFB 1000 yapılınca parça yeni köşeye kaydı (z = 1300, x = 1000).
- Şablonun kendi KA-02 kutuları aynı yerde durur; gerçek parça kullanırken onları silin ya da
  gizleyin.

0.1.3 ve 0.1.4 deneme sonuçları: 7.8. 0.1.5 düzeltmeleri: 7.9.

## 10. İndirme yerleri

GitHub: `erkancam01/solidcraft` > **Actions**.

| Ne | İş akışı (workflow) | Artifact adı |
|---|---|---|
| Aksesuar eklentisi | **Aksesuar eklentisi** (`aksesuar-eklentisi` dalına push ya da Run workflow) | `aksesuar-eklentisi` |
| Türkçe Windows SolidCraft | **Turkce Windows** | `solidcraft-turkce-windows-portable` |

Çalışmanın sayfasının en altında **Artifacts** bölümünden indirilir (GitHub'a giriş gerekir;
30 gün saklanır).

Eklenti 0.1.5 ve Aksesuar Kütüphanesi yalnızca `aksesuar-yerlesim` dalında. **Run workflow**
derken dal olarak `aksesuar-yerlesim`'i seçin; `aksesuar-eklentisi` dalı 0.1.2 verir.

## 11. Bilinen sınırlar

- Kanala Yerleştir'de mesafe oturma yüzünün ortasına göredir; köşe parçalarında payı elle
  eklemek gerekir (bkz. 4.1). Parça ters durursa **180° çevir**'i deneyin.
- Eklenti kurulduktan ya da güncellendikten sonra izinleri (**cikti** klasörü) kullanıcı kendisi
  onaylar (Eklenti yükle… onay ekranı); izin otomatik verilmez. Kontrol kanalından aynı oturumda
  ikinci `plugin.install` bir kez takılmıştı; uygulamayı yeniden başlatıp kurmak çözdü.
- Kanala Yerleştir (eklenti komutu), kanal tabanının köşesinde iki kenarın dik kesiştiğini
  (dikdörtgen taban) varsayar. Yan konum (kanal ortası) yerleştirme anındaki taban genişliğinden
  alınır. (Yerleşim noktaları gönyeli köşeyi 0.1.5'ten beri destekler, 7.9.)
- Tij listesi bileşen yerleşimlerini ayrı saymaz: her gövde bir kez sayılır (bileşen listesi
  okuma-yalnız eklentilere açık değil).
- Seçim, girdi penceresinde çizilemediği için komut **o anki seçimi** okur: önce seçin, sonra
  komuta tıklayın.
- Köşe aktarmanın kartındaki bağlantı noktası **deneme verisidir**; yeniden tanımlayın (7.2).
- Yerleştirme kartındaki **Ters çevir** ve **180° çevir** köşe aktarmalarda işe yaramaz. Yön
  yanlışsa bağlantı noktasını yeniden tanımlayın.
- `assembly.bom` (malzeme listesi komutu) bastırılmış parçaları da sayar. Eklentinin Tij ve
  Malzeme Listesi CSV'si saymaz.
- Eklenti henüz kütüphane panelinden başlatılamaz; **Eklentiler** menüsünden ya da **Model >
  Eklentiler** grubundan çalıştırın.
