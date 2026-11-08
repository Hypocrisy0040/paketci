# paketçi

[English](#english) · [Türkçe](#türkçe)

## English

A single terminal interface (TUI) for package management on Linux. Paketçi detects your
distribution and uses its package manager, plus an AUR helper, Flatpak and Snap when
available, to search, install, remove and update packages from one screen.

### Installation

Download `paketci-<version>.tar.gz` from the [latest release](https://github.com/Hypocrisy0040/paketci/releases/latest).
Extract the archive, open a terminal inside the `Paketci Installation File` folder and run:

```sh
sh Install.sh
```

The script installs missing build tools (it asks for your `sudo`/`doas` password only for
this step), compiles the program on your computer and installs it to `~/.local/bin/paketci`.
If Rust 1.74+ is not available, it downloads a temporary copy and removes it afterwards.
Then open Paketçi from your application menu, or run `paketci` or `paketci firefox` in a terminal.

**Update:** Extract the new version and run `sh Install.sh` from it. The new version replaces
the old one. If compilation fails, the old version stays in place.

**Uninstall:** `sh Install.sh --uninstall`

### Features

- **Distribution detection:** Arch (pacman), Debian/Ubuntu (apt), Fedora/RHEL (dnf),
  openSUSE (zypper), Alpine (apk), Void (xbps).
- **Forgiving search** across distribution repositories, AUR (paru/yay/pikaur), Flatpak and Snap:
  tolerates typos (nearby keyboard keys count less), Turkish letters (`fırefox`) and separators
  (`libre office`), knows common names (`vscode` → `code`), finds the package providing a command
  (`convert` → `imagemagick`), ranks development and documentation packages lower, and slightly
  favors packages you have installed through Paketçi (stored in `~/.local/state/paketci/history`).
- **Install, remove and update** with multi-select (`Space`), an installed-package list and filter.
- **Runs inside the interface:** `sudo` password prompts, progress bars and confirmations appear in
  an embedded terminal, after a confirmation window shows the commands.
- **Impact preview:** The confirmation window lists packages that will also be installed or removed,
  and the download size.
- **Update check at startup**, with the pending count shown on the tab.
- **Selective updates:** Select packages with `Space` in the Updates tab; `Enter` updates only those.
- **Scrollback and log:** `PgUp`/`PgDn` scroll through command output, which is also written to
  `~/.local/state/paketci/paketci.log`. Passwords are not recorded.
- **Restart notice** in the title bar after updates such as a new kernel.
- **Maintenance tab:** repair a broken installation, export and import your package list,
  clean caches, remove orphans, add Flathub, choose the language.
- **Turkish and English:** Detected from your system (Turkish if your language is Turkish, otherwise
  English), asked on first launch if unknown, and changeable in the Maintenance tab.

### Shortcuts

| Key | Action |
|---|---|
| `1-4` / `Tab` | Search · Installed · Updates · Maintenance |
| `/` | Search/filter box (`Enter`: search) |
| `↑↓` `j k` | Move |
| `Space` | Select / deselect |
| `Enter` / `i` | Install (Search) · Remove (Installed, also `r`) |
| `Enter` | Updates tab: update selected, or all if none selected |
| `s` | Source filter |
| `d` | Details |
| `u` | Update everything |
| `c` / `R` | Check for updates / refresh repositories |
| `PgUp` `PgDn` | Scroll command output |
| `?` `q` | Help · Quit |

### Known limitations

- End-to-end testing has so far been done on one Linux system. Other distributions are covered
  by parser tests only. Please report output that is not parsed correctly.
- **Arch:** Repository packages cannot be updated individually, because partial upgrades can break
  the system; `pacman -Sy` is never used. The repair option only checks the package database.
- **Fedora and openSUSE:** No impact preview, because `dnf` and `zypper` need root even to simulate.
  They show their own summary during the operation.
- **Flatpak and Snap:** The impact preview does not include dependencies.
- **Restart notice:** Not available on Void. On Debian/Ubuntu it depends on `/run/reboot-required`,
  which minimal Debian installs may not create.
- **Command search:** Needs `apt-file` on Debian/Ubuntu and an up-to-date `pacman -Fy` database on
  Arch. Not available on Void.
- **Import:** If a package in the list is not in your repositories, tools such as `apt` may reject
  the whole operation.
- Windows and macOS are not supported.

### Project layout

```
Paketci Installation File/
  Install.sh           install, update and uninstall
  Installation File/   Cargo.toml, Cargo.lock, README.md, LICENSE, src/
```

### License

Copyright (C) 2026 Hypocrisy0040

This program is free software: you can redistribute it and/or modify it under the terms of the
GNU General Public License as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version. It is distributed WITHOUT ANY WARRANTY. See
[LICENSE](LICENSE) for details.

---

## Türkçe

Linux için tek arayüzlü paket yöneticisi (TUI). Paketçi dağıtımınızı algılar; o dağıtımın paket
yöneticisini ve varsa AUR yardımcısı, Flatpak ve Snap'i kullanarak arama, kurma, kaldırma ve
güncelleme işlemlerini tek ekrandan yaptırır.

### Kurulum

`paketci-<sürüm>.tar.gz` dosyasını [son sürüm](https://github.com/Hypocrisy0040/paketci/releases/latest) sayfasından indirin.
Arşivi açın, `Paketci Installation File` klasörünün içinde terminalde şunu çalıştırın:

```sh
sh Install.sh
```

Betik eksik derleme araçlarını kurar (yalnızca bu adımda `sudo`/`doas` parolası sorar), programı
bilgisayarınızda derler ve `~/.local/bin/paketci` konumuna kurar. Rust 1.74+ yoksa geçici bir kopya
indirir ve sonunda siler. Ardından Paketçi'yi uygulama menüsünden açın ya da terminalde `paketci`
veya `paketci firefox` yazın.

**Güncelleme:** Yeni sürümün arşivini açıp içindeki `sh Install.sh` komutunu çalıştırın. Eski sürümün
yerine yenisi geçer. Derleme başarısız olursa eski sürüm yerinde kalır.

**Kaldırma:** `sh Install.sh --uninstall`

### Özellikler

- **Dağıtım algılama:** Arch (pacman), Debian/Ubuntu (apt), Fedora/RHEL (dnf), openSUSE (zypper),
  Alpine (apk), Void (xbps).
- **Hatalara dayanıklı arama:** Dağıtım depoları, AUR (paru/yay/pikaur), Flatpak ve Snap'te. Yazım
  hatalarını (yakın klavye tuşları daha az sayılır), Türkçe harfleri (`fırefox`) ve ayraçları
  (`libre office`) tolere eder, yaygın adları bilir (`vscode` → `code`), bir komutu sağlayan paketi
  bulur (`convert` → `imagemagick`), geliştirme ve belge paketlerini alta alır, Paketçi ile kurduğunuz
  paketleri biraz öne çıkarır (`~/.local/state/paketci/history` dosyasında tutulur).
- **Kurma, kaldırma ve güncelleme:** Çoklu seçim (`Space`), kurulu paket listesi ve filtresi.
- **Arayüzden çıkmadan çalışma:** `sudo` parolası, ilerleme çubukları ve onay soruları gömülü
  terminalde görünür; komutlar önce onay penceresinde gösterilir.
- **Etki önizlemesi:** Onay penceresinde birlikte kurulacak veya kaldırılacak paketler ve indirme boyutu.
- **Açılışta güncelleme denetimi:** Bekleyen güncelleme sayısı sekmede görünür.
- **Seçerek güncelleme:** Güncellemeler sekmesinde `Space` ile seçilir, `Enter` yalnızca seçilenleri günceller.
- **Geriye kaydırma ve günlük:** `PgUp`/`PgDn` ile komut çıktısında gezilir; çıktı
  `~/.local/state/paketci/paketci.log` dosyasına da yazılır. Parolalar kaydedilmez.
- **Yeniden başlatma uyarısı:** Yeni çekirdek gibi güncellemelerden sonra üst başlıkta görünür.
- **Bakım sekmesi:** Bozuk kurulumu onarma, paket listesini dışa/içe aktarma, önbellek ve yetim paket
  temizliği, Flathub ekleme, dil seçimi.
- **Türkçe ve İngilizce:** Sistem dilinden algılanır (Türkçe ise Türkçe, değilse İngilizce), bulunamazsa
  ilk açılışta sorulur, Bakım sekmesinden değiştirilebilir.

### Kısayollar

| Tuş | İşlev |
|---|---|
| `1-4` / `Tab` | Ara · Kurulu · Güncellemeler · Bakım |
| `/` | Arama/filtre kutusu (`Enter`: ara) |
| `↑↓` `j k` | Gezin |
| `Space` | Seç / seçimi kaldır |
| `Enter` / `i` | Kur (Ara) · Kaldır (Kurulu, `r` de olur) |
| `Enter` | Güncellemeler sekmesinde seçilileri, seçim yoksa hepsini güncelle |
| `s` | Kaynak filtresi |
| `d` | Ayrıntı |
| `u` | Her şeyi güncelle |
| `c` / `R` | Güncellemeleri denetle / depoları yenile |
| `PgUp` `PgDn` | Komut çıktısında geriye kaydır |
| `?` `q` | Yardım · Çıkış |

### Bilinen sınırlar

- Uçtan uca deneme şimdilik tek bir Linux sisteminde yapıldı. Diğer dağıtımlar yalnızca ayrıştırıcı
  testleriyle doğrulandı. Yanlış okunan çıktıları lütfen bildirin.
- **Arch:** Kısmi güncelleme sistemi bozabileceği için depo paketleri tek tek güncellenemez; `pacman -Sy`
  hiç kullanılmaz. Onarma seçeneği yalnızca paket veritabanını denetler.
- **Fedora ve openSUSE:** `dnf` ve `zypper` simülasyon için de root istediğinden etki önizlemesi yoktur;
  özeti işlem sırasında kendileri gösterir.
- **Flatpak ve Snap:** Etki önizlemesinde bağımlılıklar görünmez.
- **Yeniden başlatma uyarısı:** Void'de yoktur. Debian/Ubuntu'da `/run/reboot-required` dosyasına bağlıdır;
  sade Debian kurulumları bu dosyayı oluşturmayabilir.
- **Komut araması:** Debian/Ubuntu'da `apt-file`, Arch'ta güncel bir `pacman -Fy` veritabanı gerekir.
  Void'de yoktur.
- **İçe aktarma:** Listedeki bir paket depolarınızda yoksa `apt` gibi araçlar işlemin tamamını reddedebilir.
- Windows ve macOS desteklenmez.

### Lisans

Telif hakkı (C) 2026 Hypocrisy0040

Bu program özgür yazılımdır: Free Software Foundation tarafından yayınlanan GNU Genel Kamu Lisansı'nın
3. sürümü veya (tercihinize göre) daha sonraki bir sürümü kapsamında yeniden dağıtabilir ve
değiştirebilirsiniz. Hiçbir GARANTİ VERİLMEZ. Ayrıntılar için [LICENSE](LICENSE) dosyasına bakın.
