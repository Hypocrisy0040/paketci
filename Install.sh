#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Hypocrisy0040
#
# paketçi kurulum ve güncelleme betiği.
#
#   sh Install.sh               derler ve kurar; eski sürüm varsa yenisiyle değiştirir
#   sh Install.sh --uninstall   kurulu dosyaları siler (--kaldir ile aynı)
#
# Güncelleme: yeni arşivdeki Install.sh çalıştırılır. Program önce geçici klasörde
# derlenir ve denenir; eski sürüm ancak ondan sonra değiştirilir. Kurulan
# dosyalar LISTE dosyasına yazılır, böylece sonraki bir sürüm artık kullanmadığı
# dosyaları da temizler. Yeni bir dosya kurulursa liste_guncelle içine eklenmeli.

set -eu

UYGULAMA=paketci
EN_AZ_RUST=1.74.0

: "${HOME:?HOME is not set}"
KAYNAK=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)/"Installation File"
BIN_DIR=$HOME/.local/bin
VERI=${XDG_DATA_HOME:-$HOME/.local/share}
ONBELLEK=${XDG_CACHE_HOME:-$HOME/.cache}
AYARLAR=${XDG_CONFIG_HOME:-$HOME/.config}/$UYGULAMA
HEDEF=$BIN_DIR/$UYGULAMA
MENU_DOSYASI=$VERI/applications/$UYGULAMA.desktop
SIMGE=$VERI/icons/hicolor/scalable/apps/$UYGULAMA.svg
LISTE=$VERI/$UYGULAMA/kurulan-dosyalar.txt

GECICI=
YENI_DOSYA=
DIL=en

if [ -t 1 ]; then
    KALIN=$(printf '\033[1m')
    SARI=$(printf '\033[1;33m')
    KIRMIZI=$(printf '\033[1;31m')
    SIFIR=$(printf '\033[0m')
else
    KALIN=
    SARI=
    KIRMIZI=
    SIFIR=
fi

# Programla aynı kural: kayıtlı seçim, sonra sistem dili; Türkçe değilse İngilizce.
dil_belirle() {
    deger=$(sed -n 's/^lang=//p' "$AYARLAR/config" 2>/dev/null | head -n 1) || true
    if [ -z "$deger" ]; then
        for aday in "${LANGUAGE:-}" "${LC_ALL:-}" "${LC_MESSAGES:-}" "${LANG:-}"; do
            case $aday in
                "" | C | C.* | C:* | POSIX) ;;
                *)
                    deger=$aday
                    break
                    ;;
            esac
        done
    fi
    case $deger in
        tr | tr[_.@:-]*) DIL=tr ;;
    esac
}
dil_belirle

metin() {
    if [ "$DIL" = tr ]; then
        printf '%s' "$1"
    else
        printf '%s' "$2"
    fi
}

bilgi() { printf '%s==>%s %s\n' "$KALIN" "$SIFIR" "$(metin "$1" "$2")"; }
uyari() { printf '%s%s%s %s\n' "$SARI" "$(metin 'Uyarı:' 'Warning:')" "$SIFIR" "$(metin "$1" "$2")" >&2; }
hata() {
    printf '%s%s%s %s\n' "$KIRMIZI" "$(metin 'Hata:' 'Error:')" "$SIFIR" "$(metin "$1" "$2")" >&2
    exit 1
}

temizle() {
    if [ -n "$YENI_DOSYA" ]; then
        rm -f -- "$YENI_DOSYA"
    fi
    if [ -n "$GECICI" ]; then
        rm -rf -- "$GECICI"
    fi
}
trap temizle EXIT
trap 'exit 130' HUP INT TERM

komut_var() { command -v "$1" >/dev/null 2>&1; }

# $1 sürümü $2 sürümüne eşit veya ondan yeniyse başarılı döner.
surum_en_az() {
    awk -v a="$1" -v b="$2" 'BEGIN {
        n = split(a, x, "."); m = split(b, y, ".")
        for (i = 1; i <= (n > m ? n : m); i++) {
            if (x[i] + 0 > y[i] + 0) exit 0
            if (x[i] + 0 < y[i] + 0) exit 1
        }
        exit 0
    }'
}

# Yalnızca bu betiğin kurduğu klasörlerdeki dosyaları siler.
guvenli_sil() {
    case $1 in
        */../*) ;;
        "$BIN_DIR"/* | "$VERI"/*) rm -f -- "$1" ;;
    esac
}

menu_onbellegini_yenile() {
    if komut_var update-desktop-database; then
        update-desktop-database "$VERI/applications" >/dev/null 2>&1 || true
    fi
}

ortami_denetle() {
    [ "$(uname -s)" = Linux ] || hata "paketçi yalnızca Linux'ta çalışır." "paketci only runs on Linux."
    if [ "$(id -u)" -eq 0 ]; then
        hata "Bu betiği root olarak veya sudo ile çalıştırma; normal kullanıcınla 'sh Install.sh' yaz. Gerekirse parolanı kendisi sorar." \
            "Do not run this script as root or with sudo; run 'sh Install.sh' as your normal user. It asks for your password if needed."
    fi
}

# Yetki gerektiren bir komutu önce gösterir, sonra çalıştırır.
yetkili() {
    if komut_var sudo; then
        yetki=sudo
    elif komut_var doas; then
        yetki=doas
    else
        hata "Eksik araçları kurmak için sudo veya doas gerekiyor: $*" "sudo or doas is required to install missing tools: $*"
    fi
    printf '%s$ %s %s%s\n' "$KALIN" "$yetki" "$*" "$SIFIR"
    "$yetki" "$@" || hata "Komut başarısız oldu: $*. Paket listelerin eskiyse sistemini güncelleyip tekrar dene." \
        "Command failed: $*. If your package lists are outdated, update your system and try again."
}

araclari_kur() {
    bilgi "Eksik derleme araçları kuruluyor." "Installing missing build tools."
    if komut_var pacman; then
        yetkili pacman -S --needed --noconfirm base-devel curl
    elif komut_var apt-get; then
        yetkili apt-get update
        yetkili apt-get install -y build-essential curl ca-certificates
    elif komut_var dnf; then
        yetkili dnf install -y gcc curl ca-certificates
    elif komut_var zypper; then
        yetkili zypper --non-interactive install gcc curl ca-certificates
    elif komut_var apk; then
        yetkili apk add build-base curl ca-certificates
    elif komut_var xbps-install; then
        yetkili xbps-install -Sy base-devel curl ca-certificates
    else
        hata "Dağıtımın tanınmadı. Bir C derleyicisi (cc) ve curl kurup tekrar dene." \
            "Your distribution was not recognized. Install a C compiler (cc) and curl, then try again."
    fi
}

derleyici_calisiyor() {
    komut_var cc || return 1
    printf 'int main(void) { return 0; }\n' >"$GECICI/deneme.c"
    cc "$GECICI/deneme.c" -o "$GECICI/deneme" >/dev/null 2>&1
}

rust_uygun() {
    komut_var cargo && komut_var rustc || return 1
    cargo_surum=$(cargo --version 2>/dev/null | awk '{print $2}') || true
    rustc_surum=$(rustc --version 2>/dev/null | awk '{print $2}') || true
    [ -n "$cargo_surum" ] && [ -n "$rustc_surum" ] || return 1
    surum_en_az "$cargo_surum" "$EN_AZ_RUST" && surum_en_az "$rustc_surum" "$EN_AZ_RUST"
}

rust_gecici_kur() {
    bilgi "Uygun Rust bulunamadı; geçici olarak indiriliyor (kurulum sonunda silinecek)." \
        "No suitable Rust found; downloading a temporary copy (removed after installation)."
    RUSTUP_HOME=$GECICI/rustup
    CARGO_HOME=$GECICI/cargo
    export RUSTUP_HOME CARGO_HOME
    unset RUSTUP_TOOLCHAIN || true
    mkdir -p "$GECICI/tmp"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "$GECICI/rustup-init.sh" \
        || hata "Rust kurulum aracı indirilemedi. İnternet bağlantını kontrol et." "Could not download the Rust installer. Check your internet connection."
    TMPDIR=$GECICI/tmp sh "$GECICI/rustup-init.sh" -y --no-modify-path --profile minimal --default-toolchain stable \
        || hata "Rust indirilemedi." "Could not download Rust."
    PATH=$CARGO_HOME/bin:$PATH
}

# Derleme bitip program denenmeden eski sürüme dokunulmaz.
programi_degistir() {
    mkdir -p "$BIN_DIR"
    YENI_DOSYA=$BIN_DIR/.$UYGULAMA.yeni.$$
    cp "$GECICI/target/release/$UYGULAMA" "$YENI_DOSYA"
    chmod 755 "$YENI_DOSYA"
    calisan_surum=$("$YENI_DOSYA" --version 2>/dev/null | awk '{print $2}') || true
    [ "$calisan_surum" = "$YENI_SURUM" ] \
        || hata "Derlenen program çalışmadı. Mevcut kurulumun değiştirilmedi." "The compiled program did not run. Your current installation was not changed."
    mv -f "$YENI_DOSYA" "$HEDEF"
    YENI_DOSYA=
}

# Kurulumu çalıştıran terminal pencere kimliği atamayı destekliyorsa menü girişi
# onu Paketçi kimliğiyle açar.
menu_komutunu_belirle() {
    terminal=
    if [ -n "${KITTY_WINDOW_ID:-}" ]; then
        terminal=kitty
    elif [ -n "${ALACRITTY_WINDOW_ID:-}${ALACRITTY_SOCKET:-}" ]; then
        terminal=alacritty
    elif [ -n "${WEZTERM_PANE:-}" ]; then
        terminal=wezterm
    elif [ -n "${XTERM_VERSION:-}" ]; then
        terminal=xterm
    else
        case ${TERM:-} in
            foot*) terminal=foot ;;
            *) terminal=$(basename -- "${TERMINAL:-yok}") ;;
        esac
    fi

    MENU_EXEC="\"$HEDEF\""
    MENU_TERMINAL=true
    if komut_var "$terminal"; then
        case $terminal in
            kitty) MENU_EXEC="kitty --class=$UYGULAMA --title=Paketçi \"$HEDEF\"" ;;
            alacritty) MENU_EXEC="alacritty --class $UYGULAMA --title Paketçi -e \"$HEDEF\"" ;;
            foot) MENU_EXEC="foot --app-id=$UYGULAMA --title=Paketçi \"$HEDEF\"" ;;
            wezterm) MENU_EXEC="wezterm start --class $UYGULAMA -- \"$HEDEF\"" ;;
            xterm) MENU_EXEC="xterm -class $UYGULAMA -T Paketçi -e \"$HEDEF\"" ;;
        esac
    fi
    if [ "$MENU_EXEC" != "\"$HEDEF\"" ]; then
        MENU_TERMINAL=false
    fi
}

menu_girisini_kur() {
    menu_komutunu_belirle
    mkdir -p "$(dirname -- "$MENU_DOSYASI")" "$(dirname -- "$SIMGE")"
    cat >"$SIMGE" <<'SVG'
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <rect x="8" y="8" width="112" height="112" rx="24" fill="#1f6feb"/>
  <path d="M64 26 100 44v40L64 102 28 84V44z" fill="#f4f7fb"/>
  <path d="M64 62 28 44l36-18 36 18z" fill="#ffc94d"/>
  <path d="M64 62v40" stroke="#1f6feb" stroke-width="5"/>
</svg>
SVG
    cat >"$MENU_DOSYASI" <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=Paketçi
GenericName=Package Manager
GenericName[tr]=Paket Yöneticisi
Comment=Search, install, remove and update Linux packages from one screen
Comment[tr]=Linux paketlerini tek ekrandan ara, kur, kaldır ve güncelle
Exec=$MENU_EXEC
Icon=$SIMGE
Terminal=$MENU_TERMINAL
StartupWMClass=$UYGULAMA
Categories=System;Settings;PackageManager;
Keywords=paket;package;apt;pacman;dnf;flatpak;snap;
EOF
    menu_onbellegini_yenile
}

# Önceki sürümün kurup bu sürümde artık kullanılmayan dosyalarını siler.
liste_guncelle() {
    yeni_liste=$GECICI/liste
    printf '%s\n' "$HEDEF" "$MENU_DOSYASI" "$SIMGE" >"$yeni_liste"
    if [ -f "$LISTE" ]; then
        while IFS= read -r dosya; do
            grep -Fxq -- "$dosya" "$yeni_liste" || guvenli_sil "$dosya"
        done <"$LISTE"
    fi
    mkdir -p "$(dirname -- "$LISTE")"
    cp "$yeni_liste" "$LISTE"
}

yol_uyarilari() {
    case :$PATH: in
        *":$BIN_DIR:"*)
            bulunan=$(command -v "$UYGULAMA" 2>/dev/null || true)
            if [ -n "$bulunan" ] && [ "$bulunan" != "$HEDEF" ]; then
                uyari "Terminalde önce başka bir kopya açılıyor: $bulunan. Yeni sürümü kullanmak için o dosyayı silebilirsin. Menüdeki Paketçi yeni sürümü açar." \
                    "Another copy runs first in the terminal: $bulunan. Delete it to use the new version. The menu entry opens the new version."
            fi
            ;;
        *)
            uyari "$BIN_DIR, PATH içinde değil. Menüdeki Paketçi çalışır; terminalden 'paketci' yazabilmek için kabuk ayarlarına şunu ekle: export PATH=\"\$HOME/.local/bin:\$PATH\"" \
                "$BIN_DIR is not in PATH. The menu entry works; to run 'paketci' from a terminal, add this to your shell settings: export PATH=\"\$HOME/.local/bin:\$PATH\""
            ;;
    esac
}

kur() {
    ortami_denetle
    if [ ! -f "$KAYNAK/Cargo.toml" ] || [ ! -d "$KAYNAK/src" ]; then
        hata "\"Installation File\" klasörü Install.sh ile aynı yerde olmalı." "The \"Installation File\" folder must be next to Install.sh."
    fi
    YENI_SURUM=$(sed -n 's/^version *= *"\([^"]*\)".*/\1/p' "$KAYNAK/Cargo.toml" | head -n 1)
    [ -n "$YENI_SURUM" ] || hata "Cargo.toml içinde sürüm bulunamadı." "No version found in Cargo.toml."

    ESKI_SURUM=
    if [ -e "$HEDEF" ]; then
        ESKI_SURUM=$("$HEDEF" --version 2>/dev/null | awk '{print $2}') || true
        [ -n "$ESKI_SURUM" ] || ESKI_SURUM='?'
        bilgi "Kurulu sürüm: $ESKI_SURUM, arşivdeki sürüm: $YENI_SURUM" "Installed version: $ESKI_SURUM, archive version: $YENI_SURUM"
        if [ "$ESKI_SURUM" != '?' ] && ! surum_en_az "$YENI_SURUM" "$ESKI_SURUM"; then
            uyari "Arşivdeki sürüm kurulu olandan eski; kurulu sürümün yerine bu eski sürüm geçecek." \
                "The archive version is older than the installed one; the older version will replace it."
        fi
    else
        bilgi "Paketçi $YENI_SURUM kuruluyor." "Installing Paketçi $YENI_SURUM."
    fi

    mkdir -p "$ONBELLEK"
    GECICI=$(mktemp -d "$ONBELLEK/$UYGULAMA-kurulum.XXXXXX")

    kullanici_cargo=${CARGO_HOME:-$HOME/.cargo}/bin
    if [ -x "$kullanici_cargo/cargo" ]; then
        PATH=$kullanici_cargo:$PATH
    fi

    if ! derleyici_calisiyor || { ! rust_uygun && ! komut_var curl; }; then
        araclari_kur
        derleyici_calisiyor || hata "C derleyicisi kurulduktan sonra da çalışmadı." "The C compiler still does not work after installation."
    fi
    if ! rust_uygun; then
        rust_gecici_kur
    fi

    bilgi "Derleniyor; bu birkaç dakika sürebilir." "Compiling; this may take a few minutes."
    CARGO_TARGET_DIR=$GECICI/target cargo build --release --locked --manifest-path "$KAYNAK/Cargo.toml" \
        || hata "Derleme başarısız oldu. Mevcut kurulumun değiştirilmedi." "Compilation failed. Your current installation was not changed."

    programi_degistir
    menu_girisini_kur
    liste_guncelle

    if [ -z "$ESKI_SURUM" ]; then
        bilgi "Paketçi $YENI_SURUM kuruldu." "Paketçi $YENI_SURUM installed."
    elif [ "$ESKI_SURUM" = "$YENI_SURUM" ]; then
        bilgi "Paketçi $YENI_SURUM yeniden kuruldu." "Paketçi $YENI_SURUM reinstalled."
    else
        bilgi "Paketçi güncellendi: $ESKI_SURUM -> $YENI_SURUM" "Paketçi updated: $ESKI_SURUM -> $YENI_SURUM"
    fi
    yol_uyarilari
    bilgi "Uygulama menüsünden Paketçi'yi açabilirsin." "You can open Paketçi from your application menu."
}

kaldir() {
    ortami_denetle
    if [ ! -e "$HEDEF" ] && [ ! -f "$LISTE" ]; then
        bilgi "Paketçi kurulu görünmüyor." "Paketçi does not appear to be installed."
        return 0
    fi
    if [ -f "$LISTE" ]; then
        while IFS= read -r dosya; do
            guvenli_sil "$dosya"
        done <"$LISTE"
    fi
    guvenli_sil "$HEDEF"
    guvenli_sil "$MENU_DOSYASI"
    guvenli_sil "$SIMGE"
    guvenli_sil "$LISTE"
    rmdir "$(dirname -- "$LISTE")" 2>/dev/null || true
    # Dil seçimi ve işlem günlüğü
    rm -rf -- "$AYARLAR" "${XDG_STATE_HOME:-$HOME/.local/state}/$UYGULAMA"
    menu_onbellegini_yenile
    bilgi "Paketçi kaldırıldı." "Paketçi removed."
}

case ${1:-} in
    "") kur ;;
    --uninstall | --kaldir) kaldir ;;
    -h | --help | --yardim)
        printf '%s\n' "$(metin 'Kullanım: sh Install.sh [--uninstall]' 'Usage: sh Install.sh [--uninstall]')"
        ;;
    *) hata "Bilinmeyen seçenek: $1 (kullanım: sh Install.sh [--uninstall])" "Unknown option: $1 (usage: sh Install.sh [--uninstall])" ;;
esac
