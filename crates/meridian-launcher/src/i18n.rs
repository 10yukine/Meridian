//! The launcher's languages: English is built in; any other language in [`Language::ALL`] is
//! downloaded on demand from the pinned manifest (`assets/launcher-languages.json`), checked
//! against its SHA-256 before it is saved under `launcher-lang/<code>.json`, and falls back to
//! English when missing or while downloading.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    collections::HashMap,
    fs,
    io::{self, Read},
    path::Path,
    sync::{OnceLock, RwLock},
    time::Duration,
};

const MANIFEST: &str = include_str!("../assets/launcher-languages.json");
const SAVED_FILE: &str = "launcher-language.txt";
const LANG_DIR: &str = "launcher-lang";
const RAW_BASE: &str = "https://raw.githubusercontent.com/10yukine/Meridian";
/// Larger than any language table: a file claiming more is rejected.
const MAX_FILE: u64 = 1 << 20;
/// Current Minecraft 26.3 options.txt data version, so a fresh options.txt with `lang:` runs no
/// legacy `< 4892` data fixers.
const OPTIONS_DATA_VERSION: i32 = 4892;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "fr")]
    French,
    #[serde(rename = "ru")]
    Russian,
    #[serde(rename = "de")]
    German,
    #[serde(rename = "zh")]
    Chinese,
    #[serde(rename = "es")]
    Spanish,
    #[serde(rename = "uk")]
    Ukrainian,
    #[serde(rename = "pl")]
    Polish,
    #[serde(rename = "pt-br")]
    PortugueseBrazil,
    #[serde(rename = "ja")]
    Japanese,
    #[serde(rename = "tr")]
    Turkish,
    #[serde(rename = "ko")]
    Korean,
    #[serde(rename = "hr")]
    Croatian,
    #[serde(rename = "ar")]
    Arabic,
    #[serde(rename = "hi")]
    Hindi,
    #[serde(rename = "id")]
    Indonesian,
    #[serde(rename = "pt")]
    Portuguese,
    #[serde(rename = "he")]
    Hebrew,
    #[serde(rename = "hu")]
    Hungarian,
    #[serde(rename = "ro")]
    Romanian,
    #[serde(rename = "da")]
    Danish,
    #[serde(rename = "fi")]
    Finnish,
    #[serde(rename = "no")]
    Norwegian,
    #[serde(rename = "bg")]
    Bulgarian,
    #[serde(rename = "sk")]
    Slovak,
    #[serde(rename = "lt")]
    Lithuanian,
    #[serde(rename = "it")]
    Italian,
    #[serde(rename = "nl")]
    Dutch,
    #[serde(rename = "vi")]
    Vietnamese,
    #[serde(rename = "th")]
    Thai,
    #[serde(rename = "ur")]
    Urdu,
    #[serde(rename = "cs")]
    Czech,
    #[serde(rename = "sv")]
    Swedish,
    #[serde(rename = "el")]
    Greek,
    #[serde(rename = "fa")]
    Persian,
    #[serde(rename = "ca")]
    Catalan,
    #[serde(rename = "et")]
    Estonian,
    #[serde(rename = "lv")]
    Latvian,
    #[serde(rename = "sl")]
    Slovenian,
    #[serde(rename = "sr")]
    Serbian,
    #[serde(rename = "is")]
    Icelandic,
    #[serde(rename = "af")]
    Afrikaans,
    #[serde(rename = "ms")]
    Malay,
    #[serde(rename = "be")]
    Belarusian,
    #[serde(rename = "az")]
    Azerbaijani,
    #[serde(rename = "uz")]
    Uzbek,
    #[serde(rename = "kk")]
    Kazakh,
    #[serde(rename = "ka")]
    Georgian,
    #[serde(rename = "hy")]
    Armenian,
    #[serde(rename = "bs")]
    Bosnian,
    #[serde(rename = "mk")]
    Macedonian,
    #[serde(rename = "sq")]
    Albanian,
    #[serde(rename = "cy")]
    Welsh,
    #[serde(rename = "ga")]
    Irish,
    #[serde(rename = "eu")]
    Basque,
    #[serde(rename = "gl")]
    Galician,
    #[serde(rename = "fil")]
    Filipino,
    #[serde(rename = "ta")]
    Tamil,
    #[serde(rename = "mn")]
    Mongolian,
    #[serde(rename = "lo")]
    Lao,
    #[serde(rename = "so")]
    Somali,
    #[serde(rename = "fy")]
    Frisian,
    #[serde(rename = "gd")]
    ScottishGaelic,
    #[serde(rename = "lb")]
    Luxembourgish,
    #[serde(rename = "ky")]
    Kyrgyz,
    #[serde(rename = "kn")]
    Kannada,
    #[serde(rename = "mt")]
    Maltese,
    #[serde(rename = "eo")]
    Esperanto,
    #[serde(rename = "haw")]
    Hawaiian,
    #[serde(rename = "ig")]
    Igbo,
    #[serde(rename = "la")]
    Latin,
    #[serde(rename = "tt")]
    Tatar,
    #[serde(rename = "yi")]
    Yiddish,
    #[serde(rename = "yo")]
    Yoruba,
    #[serde(rename = "zu")]
    Zulu,
    #[serde(rename = "ba")]
    Bashkir,
    #[serde(rename = "br")]
    Breton,
    #[serde(rename = "cv")]
    Chuvash,
    #[serde(rename = "fo")]
    Faroese,
    #[serde(rename = "fur")]
    Friulian,
    #[serde(rename = "li")]
    Limburgish,
    #[serde(rename = "lmo")]
    Lombard,
    #[serde(rename = "oc")]
    Occitan,
    #[serde(rename = "sah")]
    Sakha,
    #[serde(rename = "se")]
    NorthernSami,
    #[serde(rename = "szl")]
    Silesian,
    #[serde(rename = "vec")]
    Venetian,
}

/// Where downloading a language pack ([`crate::Backend::set_language`]) stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LanguageDownload {
    #[default]
    Idle,
    Downloading(Language),
    Failed(String),
}

impl Language {
    pub const ALL: [Language; 87] = [
        Self::English,
        Self::French,
        Self::Russian,
        Self::German,
        Self::Chinese,
        Self::Spanish,
        Self::Ukrainian,
        Self::Polish,
        Self::PortugueseBrazil,
        Self::Japanese,
        Self::Turkish,
        Self::Korean,
        Self::Croatian,
        Self::Arabic,
        Self::Hindi,
        Self::Indonesian,
        Self::Portuguese,
        Self::Hebrew,
        Self::Hungarian,
        Self::Romanian,
        Self::Danish,
        Self::Finnish,
        Self::Norwegian,
        Self::Bulgarian,
        Self::Slovak,
        Self::Lithuanian,
        Self::Italian,
        Self::Dutch,
        Self::Vietnamese,
        Self::Thai,
        Self::Urdu,
        Self::Czech,
        Self::Swedish,
        Self::Greek,
        Self::Persian,
        Self::Catalan,
        Self::Estonian,
        Self::Latvian,
        Self::Slovenian,
        Self::Serbian,
        Self::Icelandic,
        Self::Afrikaans,
        Self::Malay,
        Self::Belarusian,
        Self::Azerbaijani,
        Self::Uzbek,
        Self::Kazakh,
        Self::Georgian,
        Self::Armenian,
        Self::Bosnian,
        Self::Macedonian,
        Self::Albanian,
        Self::Welsh,
        Self::Irish,
        Self::Basque,
        Self::Galician,
        Self::Filipino,
        Self::Tamil,
        Self::Mongolian,
        Self::Lao,
        Self::Somali,
        Self::Frisian,
        Self::ScottishGaelic,
        Self::Luxembourgish,
        Self::Kyrgyz,
        Self::Kannada,
        Self::Maltese,
        Self::Esperanto,
        Self::Hawaiian,
        Self::Igbo,
        Self::Latin,
        Self::Tatar,
        Self::Yiddish,
        Self::Yoruba,
        Self::Zulu,
        Self::Bashkir,
        Self::Breton,
        Self::Chuvash,
        Self::Faroese,
        Self::Friulian,
        Self::Limburgish,
        Self::Lombard,
        Self::Occitan,
        Self::Sakha,
        Self::NorthernSami,
        Self::Silesian,
        Self::Venetian,
    ];

    /// Its code in `launcher-language.txt` and `launcher/lang/<code>.json`.
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::French => "fr",
            Self::Russian => "ru",
            Self::German => "de",
            Self::Chinese => "zh",
            Self::Spanish => "es",
            Self::Ukrainian => "uk",
            Self::Polish => "pl",
            Self::PortugueseBrazil => "pt-br",
            Self::Japanese => "ja",
            Self::Turkish => "tr",
            Self::Korean => "ko",
            Self::Croatian => "hr",
            Self::Arabic => "ar",
            Self::Hindi => "hi",
            Self::Indonesian => "id",
            Self::Portuguese => "pt",
            Self::Hebrew => "he",
            Self::Hungarian => "hu",
            Self::Romanian => "ro",
            Self::Danish => "da",
            Self::Finnish => "fi",
            Self::Norwegian => "no",
            Self::Bulgarian => "bg",
            Self::Slovak => "sk",
            Self::Lithuanian => "lt",
            Self::Italian => "it",
            Self::Dutch => "nl",
            Self::Vietnamese => "vi",
            Self::Thai => "th",
            Self::Urdu => "ur",
            Self::Czech => "cs",
            Self::Swedish => "sv",
            Self::Greek => "el",
            Self::Persian => "fa",
            Self::Catalan => "ca",
            Self::Estonian => "et",
            Self::Latvian => "lv",
            Self::Slovenian => "sl",
            Self::Serbian => "sr",
            Self::Icelandic => "is",
            Self::Afrikaans => "af",
            Self::Malay => "ms",
            Self::Belarusian => "be",
            Self::Azerbaijani => "az",
            Self::Uzbek => "uz",
            Self::Kazakh => "kk",
            Self::Georgian => "ka",
            Self::Armenian => "hy",
            Self::Bosnian => "bs",
            Self::Macedonian => "mk",
            Self::Albanian => "sq",
            Self::Welsh => "cy",
            Self::Irish => "ga",
            Self::Basque => "eu",
            Self::Galician => "gl",
            Self::Filipino => "fil",
            Self::Tamil => "ta",
            Self::Mongolian => "mn",
            Self::Lao => "lo",
            Self::Somali => "so",
            Self::Frisian => "fy",
            Self::ScottishGaelic => "gd",
            Self::Luxembourgish => "lb",
            Self::Kyrgyz => "ky",
            Self::Kannada => "kn",
            Self::Maltese => "mt",
            Self::Esperanto => "eo",
            Self::Hawaiian => "haw",
            Self::Igbo => "ig",
            Self::Latin => "la",
            Self::Tatar => "tt",
            Self::Yiddish => "yi",
            Self::Yoruba => "yo",
            Self::Zulu => "zu",
            Self::Bashkir => "ba",
            Self::Breton => "br",
            Self::Chuvash => "cv",
            Self::Faroese => "fo",
            Self::Friulian => "fur",
            Self::Limburgish => "li",
            Self::Lombard => "lmo",
            Self::Occitan => "oc",
            Self::Sakha => "sah",
            Self::NorthernSami => "se",
            Self::Silesian => "szl",
            Self::Venetian => "vec",
        }
    }

    /// Its name in itself: what a player looks for in a list of languages.
    pub fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::French => "Français",
            Self::Russian => "Русский",
            Self::German => "Deutsch",
            Self::Chinese => "简体中文",
            Self::Spanish => "Español",
            Self::Ukrainian => "Українська",
            Self::Polish => "Polski",
            Self::PortugueseBrazil => "Português (Brasil)",
            Self::Japanese => "日本語",
            Self::Turkish => "Türkçe",
            Self::Korean => "한국어",
            Self::Croatian => "Hrvatski",
            Self::Arabic => "العربية",
            Self::Hindi => "हिन्दी",
            Self::Indonesian => "Bahasa Indonesia",
            Self::Portuguese => "Português (Portugal)",
            Self::Hebrew => "עברית",
            Self::Hungarian => "Magyar",
            Self::Romanian => "Română",
            Self::Danish => "Dansk",
            Self::Finnish => "Suomi",
            Self::Norwegian => "Norsk",
            Self::Bulgarian => "Български",
            Self::Slovak => "Slovenčina",
            Self::Lithuanian => "Lietuvių",
            Self::Italian => "Italiano",
            Self::Dutch => "Nederlands",
            Self::Vietnamese => "Tiếng Việt",
            Self::Thai => "ไทย",
            Self::Urdu => "اردو",
            Self::Czech => "Čeština",
            Self::Swedish => "Svenska",
            Self::Greek => "Ελληνικά",
            Self::Persian => "فارسی",
            Self::Catalan => "Català",
            Self::Estonian => "Eesti",
            Self::Latvian => "Latviešu",
            Self::Slovenian => "Slovenščina",
            Self::Serbian => "Српски",
            Self::Icelandic => "Íslenska",
            Self::Afrikaans => "Afrikaans",
            Self::Malay => "Bahasa Melayu",
            Self::Belarusian => "Беларуская",
            Self::Azerbaijani => "Azərbaycanca",
            Self::Uzbek => "Oʻzbekcha",
            Self::Kazakh => "Қазақша",
            Self::Georgian => "ქართული",
            Self::Armenian => "Հայերեն",
            Self::Bosnian => "Bosanski",
            Self::Macedonian => "Македонски",
            Self::Albanian => "Shqip",
            Self::Welsh => "Cymraeg",
            Self::Irish => "Gaeilge",
            Self::Basque => "Euskara",
            Self::Galician => "Galego",
            Self::Filipino => "Filipino",
            Self::Tamil => "தமிழ்",
            Self::Mongolian => "Монгол",
            Self::Lao => "ລາວ",
            Self::Somali => "Soomaali",
            Self::Frisian => "Frysk",
            Self::ScottishGaelic => "Gàidhlig",
            Self::Luxembourgish => "Lëtzebuergesch",
            Self::Kyrgyz => "Кыргызча",
            Self::Kannada => "ಕನ್ನಡ",
            Self::Maltese => "Malti",
            Self::Esperanto => "Esperanto",
            Self::Hawaiian => "ʻŌlelo Hawaiʻi",
            Self::Igbo => "Igbo",
            Self::Latin => "Latina",
            Self::Tatar => "Татарча",
            Self::Yiddish => "ייִדיש",
            Self::Yoruba => "Yorùbá",
            Self::Zulu => "isiZulu",
            Self::Bashkir => "Башҡортса",
            Self::Breton => "Brezhoneg",
            Self::Chuvash => "Чӑвашла",
            Self::Faroese => "Føroyskt",
            Self::Friulian => "Furlan",
            Self::Limburgish => "Limburgs",
            Self::Lombard => "Lombard",
            Self::Occitan => "Occitan",
            Self::Sakha => "Саха тыла",
            Self::NorthernSami => "Davvisámegiella",
            Self::Silesian => "Ślōnskŏ",
            Self::Venetian => "Vèneto",
        }
    }

    /// Plain Latin name for a language whose native script the bundled Geist faces do not cover in
    /// Phase 1, so the language picker marks it unavailable without drawing missing-glyph boxes.
    pub fn english_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::French => "French",
            Self::Russian => "Russian",
            Self::German => "German",
            Self::Chinese => "Chinese",
            Self::Spanish => "Spanish",
            Self::Ukrainian => "Ukrainian",
            Self::Polish => "Polish",
            Self::PortugueseBrazil => "Portuguese (Brazil)",
            Self::Japanese => "Japanese",
            Self::Turkish => "Turkish",
            Self::Korean => "Korean",
            Self::Croatian => "Croatian",
            Self::Arabic => "Arabic",
            Self::Hindi => "Hindi",
            Self::Indonesian => "Indonesian",
            Self::Portuguese => "Portuguese (Portugal)",
            Self::Hebrew => "Hebrew",
            Self::Hungarian => "Hungarian",
            Self::Romanian => "Romanian",
            Self::Danish => "Danish",
            Self::Finnish => "Finnish",
            Self::Norwegian => "Norwegian",
            Self::Bulgarian => "Bulgarian",
            Self::Slovak => "Slovak",
            Self::Lithuanian => "Lithuanian",
            Self::Italian => "Italian",
            Self::Dutch => "Dutch",
            Self::Vietnamese => "Vietnamese",
            Self::Thai => "Thai",
            Self::Urdu => "Urdu",
            Self::Czech => "Czech",
            Self::Swedish => "Swedish",
            Self::Greek => "Greek",
            Self::Persian => "Persian",
            Self::Catalan => "Catalan",
            Self::Estonian => "Estonian",
            Self::Latvian => "Latvian",
            Self::Slovenian => "Slovenian",
            Self::Serbian => "Serbian",
            Self::Icelandic => "Icelandic",
            Self::Afrikaans => "Afrikaans",
            Self::Malay => "Malay",
            Self::Belarusian => "Belarusian",
            Self::Azerbaijani => "Azerbaijani",
            Self::Uzbek => "Uzbek",
            Self::Kazakh => "Kazakh",
            Self::Georgian => "Georgian",
            Self::Armenian => "Armenian",
            Self::Bosnian => "Bosnian",
            Self::Macedonian => "Macedonian",
            Self::Albanian => "Albanian",
            Self::Welsh => "Welsh",
            Self::Irish => "Irish",
            Self::Basque => "Basque",
            Self::Galician => "Galician",
            Self::Filipino => "Filipino",
            Self::Tamil => "Tamil",
            Self::Mongolian => "Mongolian",
            Self::Lao => "Lao",
            Self::Somali => "Somali",
            Self::Frisian => "Frisian",
            Self::ScottishGaelic => "Scottish Gaelic",
            Self::Luxembourgish => "Luxembourgish",
            Self::Kyrgyz => "Kyrgyz",
            Self::Kannada => "Kannada",
            Self::Maltese => "Maltese",
            Self::Esperanto => "Esperanto",
            Self::Hawaiian => "Hawaiian",
            Self::Igbo => "Igbo",
            Self::Latin => "Latin",
            Self::Tatar => "Tatar",
            Self::Yiddish => "Yiddish",
            Self::Yoruba => "Yoruba",
            Self::Zulu => "Zulu",
            Self::Bashkir => "Bashkir",
            Self::Breton => "Breton",
            Self::Chuvash => "Chuvash",
            Self::Faroese => "Faroese",
            Self::Friulian => "Friulian",
            Self::Limburgish => "Limburgish",
            Self::Lombard => "Lombard",
            Self::Occitan => "Occitan",
            Self::Sakha => "Sakha",
            Self::NorthernSami => "Northern Sami",
            Self::Silesian => "Silesian",
            Self::Venetian => "Venetian",
        }
    }

    /// Phase 1: whether the launcher's bundled Geist faces cover this language's script and
    /// alphabet without loading the prepared pack's font sheets.
    // ponytail: Phase 1 marks the 24 languages Geist lacks as unavailable so no missing-glyph
    // boxes are drawn; Phase 2 lifts this gate once `mobile_launcher_host` loads the prepared
    // pack's Unifont sheets (`use_full_atlas`) for non-Geist scripts.
    pub fn bundled_font_supported(self) -> bool {
        !matches!(
            self,
            Self::Chinese
                | Self::Japanese
                | Self::Korean
                | Self::Arabic
                | Self::Hindi
                | Self::Hebrew
                | Self::Thai
                | Self::Urdu
                | Self::Greek
                | Self::Persian
                | Self::Uzbek
                | Self::Georgian
                | Self::Armenian
                | Self::Tamil
                | Self::Lao
                | Self::Kannada
                | Self::Hawaiian
                | Self::Igbo
                | Self::Yiddish
                | Self::Yoruba
                | Self::Bashkir
                | Self::Chuvash
                | Self::Sakha
                | Self::Silesian
        )
    }

    /// Minecraft 26.3's matching `options.txt` `lang:` code (`assets/minecraft/lang/<code>.json`),
    /// or `None` for the two launcher languages Minecraft does not ship (`ur`, `zu`).
    pub fn game_code(self) -> Option<&'static str> {
        Some(match self {
            Self::English => "en_us",
            Self::French => "fr_fr",
            Self::Russian => "ru_ru",
            Self::German => "de_de",
            Self::Chinese => "zh_cn",
            Self::Spanish => "es_es",
            Self::Ukrainian => "uk_ua",
            Self::Polish => "pl_pl",
            Self::PortugueseBrazil => "pt_br",
            Self::Japanese => "ja_jp",
            Self::Turkish => "tr_tr",
            Self::Korean => "ko_kr",
            Self::Croatian => "hr_hr",
            Self::Arabic => "ar_sa",
            Self::Hindi => "hi_in",
            Self::Indonesian => "id_id",
            Self::Portuguese => "pt_pt",
            Self::Hebrew => "he_il",
            Self::Hungarian => "hu_hu",
            Self::Romanian => "ro_ro",
            Self::Danish => "da_dk",
            Self::Finnish => "fi_fi",
            Self::Norwegian => "no_no",
            Self::Bulgarian => "bg_bg",
            Self::Slovak => "sk_sk",
            Self::Lithuanian => "lt_lt",
            Self::Italian => "it_it",
            Self::Dutch => "nl_nl",
            Self::Vietnamese => "vi_vn",
            Self::Thai => "th_th",
            Self::Urdu => return None,
            Self::Czech => "cs_cz",
            Self::Swedish => "sv_se",
            Self::Greek => "el_gr",
            Self::Persian => "fa_ir",
            Self::Catalan => "ca_es",
            Self::Estonian => "et_ee",
            Self::Latvian => "lv_lv",
            Self::Slovenian => "sl_si",
            Self::Serbian => "sr_sp",
            Self::Icelandic => "is_is",
            Self::Afrikaans => "af_za",
            Self::Malay => "ms_my",
            Self::Belarusian => "be_by",
            Self::Azerbaijani => "az_az",
            Self::Uzbek => "uz_uz",
            Self::Kazakh => "kk_kz",
            Self::Georgian => "ka_ge",
            Self::Armenian => "hy_am",
            Self::Bosnian => "bs_ba",
            Self::Macedonian => "mk_mk",
            Self::Albanian => "sq_al",
            Self::Welsh => "cy_gb",
            Self::Irish => "ga_ie",
            Self::Basque => "eu_es",
            Self::Galician => "gl_es",
            Self::Filipino => "fil_ph",
            Self::Tamil => "ta_in",
            Self::Mongolian => "mn_mn",
            Self::Lao => "lo_la",
            Self::Somali => "so_so",
            Self::Frisian => "fy_nl",
            Self::ScottishGaelic => "gd_gb",
            Self::Luxembourgish => "lb_lu",
            Self::Kyrgyz => "ky_kg",
            Self::Kannada => "kn_in",
            Self::Maltese => "mt_mt",
            Self::Esperanto => "eo_uy",
            Self::Hawaiian => "haw_us",
            Self::Igbo => "ig_ng",
            Self::Latin => "la_la",
            Self::Tatar => "tt_ru",
            Self::Yiddish => "yi_de",
            Self::Yoruba => "yo_ng",
            Self::Zulu => return None,
            Self::Bashkir => "ba_ru",
            Self::Breton => "br_fr",
            Self::Chuvash => "cv_cu",
            Self::Faroese => "fo_fo",
            Self::Friulian => "fur_it",
            Self::Limburgish => "li_li",
            Self::Lombard => "lmo",
            Self::Occitan => "oc_fr",
            Self::Sakha => "sah_sah",
            Self::NorthernSami => "se_no",
            Self::Silesian => "szl",
            Self::Venetian => "vec_it",
        })
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|l| *l == self).unwrap_or(0)
    }

    /// Parses a language code (e.g. `fr`, `fr-CH`, `pt-BR`, or `pt_BR.UTF-8`)
    /// if the launcher supports it.
    pub fn parse(code: &str) -> Option<Self> {
        let lower = code.trim().to_ascii_lowercase();
        let clean = lower.split(['.', '@']).next().unwrap_or("");
        let normalized = clean.replace('_', "-");

        if normalized == "pt-br" {
            return Some(Self::PortugueseBrazil);
        }

        let base = match normalized.split('-').next().unwrap_or("") {
            "nb" | "nn" => "no",
            base => base,
        };

        Self::ALL
            .into_iter()
            .find(|language| language.code() == base)
    }

    /// The first of the system's preferred languages the launcher has, else English.
    pub fn pick<'a>(preferred: impl IntoIterator<Item = &'a str>) -> Self {
        preferred
            .into_iter()
            .find_map(Self::parse)
            .unwrap_or_default()
    }
}

#[derive(Deserialize)]
struct Manifest {
    revision: String,
    files: Vec<Entry>,
}

#[derive(Clone, Deserialize)]
struct Entry {
    code: String,
    sha256: String,
    size: u64,
}

fn manifest() -> &'static Manifest {
    static PARSED: OnceLock<Manifest> = OnceLock::new();
    PARSED.get_or_init(|| {
        let m: Manifest = serde_json::from_str(MANIFEST).expect("valid launcher-languages.json");
        assert!(
            m.revision.len() == 40 && m.revision.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid language manifest revision"
        );
        for e in &m.files {
            assert!(
                e.size > 0 && e.size <= MAX_FILE,
                "invalid size for {}",
                e.code
            );
            assert!(
                e.sha256.len() == 64 && e.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid sha256 for {}",
                e.code
            );
        }
        m
    })
}

fn entry_for(language: Language) -> Option<&'static Entry> {
    if language == Language::English {
        return None;
    }
    let code = language.code();
    manifest().files.iter().find(|e| e.code == code)
}

/// Pinned download size in bytes for `language` (0 for built-in English).
pub fn download_size(language: Language) -> u64 {
    entry_for(language).map_or(0, |e| e.size)
}

fn checks_out(entry: &Entry, bytes: &[u8]) -> bool {
    bytes.len() as u64 == entry.size && format!("{:x}", Sha256::digest(bytes)) == entry.sha256
}

fn pack_path(root: &Path, language: Language) -> std::path::PathBuf {
    root.join(LANG_DIR)
        .join(format!("{}.json", language.code()))
}

/// Whether `language` is already verified on disk in `root` (always true for English).
pub fn is_cached(root: &Path, language: Language) -> bool {
    let Some(entry) = entry_for(language) else {
        return true;
    };
    let Ok(file) = fs::File::open(pack_path(root, language)) else {
        return false;
    };
    let mut bytes = Vec::new();
    file.take(entry.size + 1).read_to_end(&mut bytes).is_ok() && checks_out(entry, &bytes)
}

/// Bitmask (`1u128 << lang.index()`) of languages already verified on disk in `root`, so callers
/// do not open and hash language tables on every UI frame.
pub fn cached_mask(root: &Path) -> u128 {
    let mut mask = 1u128 << Language::English.index();
    let Ok(entries) = fs::read_dir(root.join(LANG_DIR)) else {
        return mask;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(code) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Some(lang) = Language::ALL.into_iter().find(|l| l.code() == code)
            && is_cached(root, lang)
        {
            mask |= 1u128 << lang.index();
        }
    }
    mask
}

/// Loads `language`'s table from `<root>/launcher-lang/<code>.json` after checking its size and
/// SHA-256 against the pinned manifest.
pub fn load_from_disk(root: &Path, language: Language) -> Result<(), String> {
    let Some(entry) = entry_for(language) else {
        return Ok(());
    };
    let path = pack_path(root, language);
    let file = fs::File::open(&path).map_err(|e| format!("{}: {e}", language.name()))?;
    let mut bytes = Vec::new();
    file.take(entry.size + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{}: {e}", language.name()))?;
    if !checks_out(entry, &bytes) {
        return Err(format!("{} did not match its checksum.", language.name()));
    }
    load_table_bytes(language, &bytes)
}

/// Downloads `language` from the pinned GitHub revision, checks its size and SHA-256, saves it to
/// `<root>/launcher-lang/<code>.json`, and loads its table into memory.
pub fn fetch(root: &Path, language: Language) -> Result<(), String> {
    fetch_from(RAW_BASE, root, language)
}

pub fn fetch_from(base_url: &str, root: &Path, language: Language) -> Result<(), String> {
    let Some(entry) = entry_for(language) else {
        return Ok(());
    };
    if load_from_disk(root, language).is_ok() {
        return Ok(());
    }
    let url = format!(
        "{}/{}/launcher/lang/{}.json",
        base_url.trim_end_matches('/'),
        manifest().revision,
        entry.code
    );
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(20))
        .timeout_read(Duration::from_secs(30))
        .build();
    let mut bytes = Vec::new();
    agent
        .get(&url)
        .call()
        .map_err(|e| format!("Could not download {}: {e}", language.name()))?
        .into_reader()
        .take(entry.size + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Could not read {}: {e}", language.name()))?;
    if !checks_out(entry, &bytes) {
        return Err(format!("{} did not match its checksum.", language.name()));
    }
    load_table_bytes(language, &bytes)?;
    let dir = root.join(LANG_DIR);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let part = dir.join(format!("{}.part", entry.code));
    fs::write(&part, &bytes).map_err(|e| e.to_string())?;
    fs::rename(&part, pack_path(root, language)).map_err(|e| e.to_string())?;
    Ok(())
}

/// Parses and stores `json` as `language`'s translation table in memory.
pub fn load_table_bytes(language: Language, json: &[u8]) -> Result<(), String> {
    if language == Language::English {
        return Ok(());
    }
    let parsed: HashMap<String, String> = serde_json::from_slice(json)
        .map_err(|e| format!("{} is not a valid language table: {e}", language.name()))?;
    let slot = table_slot(language);
    let mut guard = slot.write().unwrap();
    if guard.is_empty() {
        *guard = parsed
            .into_iter()
            .filter(|(_, said)| !said.trim().is_empty())
            .map(|(en, said)| (en, &*Box::leak(said.into_boxed_str())))
            .collect();
    }
    Ok(())
}

/// The language saved in `root/launcher-language.txt`, or English.
pub fn saved_language(root: &Path) -> Language {
    fs::read_to_string(root.join(SAVED_FILE))
        .ok()
        .and_then(|text| Language::parse(&text))
        .filter(|lang| lang.bundled_font_supported())
        .unwrap_or(Language::English)
}

/// Saves `language` in `root/launcher-language.txt` and updates the game's `options.txt` `lang:`
/// unless the player already set a different game language there.
pub fn save_language(root: &Path, language: Language) -> io::Result<()> {
    let prev = saved_language(root);
    fs::create_dir_all(root)?;
    fs::write(root.join(SAVED_FILE), format!("{}\n", language.code()))?;
    sync_game_language(root, prev, language);
    Ok(())
}

/// Updates `lang:<game_code>` in `root/options.txt` and any existing `root/profiles/*/options.txt`
/// so Minecraft follows the launcher's language choice unless the player already set a different
/// game language there.
pub fn sync_game_language(root: &Path, prev: Language, language: Language) {
    let Some(game_code) = language.game_code() else {
        return;
    };
    let prev_code = prev.game_code();
    let root_options = root.join("options.txt");
    if root_options.is_file() || language != Language::English {
        let _ = write_options_lang(&root_options, prev_code, game_code);
    }
    if let Ok(entries) = fs::read_dir(root.join("profiles")) {
        for entry in entries.flatten() {
            let prof_options = entry.path().join("options.txt");
            if prof_options.is_file() {
                let _ = write_options_lang(&prof_options, prev_code, game_code);
            }
        }
    }
}

fn write_options_lang(path: &Path, prev_code: Option<&str>, game_code: &str) -> io::Result<()> {
    let lang_line = format!("lang:{game_code}");
    let out = match fs::read_to_string(path) {
        Ok(text) => {
            if let Some(existing) = text.lines().find_map(|l| l.strip_prefix("lang:")) {
                let existing = existing.trim();
                if !existing.is_empty()
                    && existing != "en_us"
                    && existing != game_code
                    && Some(existing) != prev_code
                {
                    return Ok(());
                }
            }
            let mut found = false;
            let mut has_version = false;
            let mut lines: Vec<String> = text
                .lines()
                .map(|line| {
                    if line.starts_with("version:") {
                        has_version = true;
                    }
                    if line.starts_with("lang:") {
                        found = true;
                        lang_line.clone()
                    } else {
                        line.to_owned()
                    }
                })
                .collect();
            if !has_version {
                lines.insert(0, format!("version:{OPTIONS_DATA_VERSION}"));
            }
            if !found {
                lines.push(lang_line);
            }
            let mut joined = lines.join("\n");
            joined.push('\n');
            joined
        }
        Err(_) => format!("version:{OPTIONS_DATA_VERSION}\n{lang_line}\n"),
    };
    fs::write(path, out)
}

type Table = HashMap<String, &'static str>;

fn table_slot(language: Language) -> &'static RwLock<Table> {
    static TABLES: OnceLock<Vec<RwLock<Table>>> = OnceLock::new();
    let tables = TABLES.get_or_init(|| {
        Language::ALL
            .iter()
            .map(|_| RwLock::new(HashMap::new()))
            .collect()
    });
    &tables[language.index()]
}

/// Whether `language`'s translation table is ready in memory (always true for English).
pub fn is_loaded(language: Language) -> bool {
    language == Language::English || !table_slot(language).read().unwrap().is_empty()
}

thread_local! {
    static CURRENT: Cell<Language> = const { Cell::new(Language::English) };
}

/// Shows the launcher in `language` on this thread from now on.
pub fn set(language: Language) {
    CURRENT.with(|c| c.set(language));
}

pub fn current() -> Language {
    CURRENT.with(Cell::get)
}

/// `en` in `language`, or `en` itself where the table is not loaded or lacks it.
pub fn tr_in<'a>(language: Language, en: &'a str) -> &'a str {
    if language == Language::English {
        return en;
    }
    table_slot(language)
        .read()
        .unwrap()
        .get(en)
        .copied()
        .unwrap_or(en)
}

/// `en` in the launcher's current language, falling back to English.
pub fn tr<'a>(en: &'a str) -> &'a str {
    tr_in(current(), en)
}

/// `en` in the launcher's current language with its `{name}` holes filled.
pub fn trf(en: &str, args: &[(&str, &str)]) -> String {
    let mut out = tr(en).to_owned();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    #[test]
    fn manifest_covers_every_non_english_language() {
        let m = manifest();
        assert_eq!(m.files.len(), Language::ALL.len() - 1);
        for lang in Language::ALL {
            if lang == Language::English {
                assert_eq!(download_size(lang), 0);
            } else {
                assert!(download_size(lang) > 10_000, "{} has no size", lang.code());
            }
        }
    }

    #[test]
    fn language_parsing_and_game_code_mapping() {
        assert_eq!(Language::parse("fr-CH"), Some(Language::French));
        assert_eq!(
            Language::parse("pt_BR.UTF-8"),
            Some(Language::PortugueseBrazil)
        );
        assert_eq!(Language::parse("pt-PT"), Some(Language::Portuguese));
        assert_eq!(Language::parse("nb-NO"), Some(Language::Norwegian));
        assert_eq!(Language::French.game_code(), Some("fr_fr"));
        assert_eq!(Language::PortugueseBrazil.game_code(), Some("pt_br"));
        assert_eq!(Language::Chinese.game_code(), Some("zh_cn"));
        assert_eq!(Language::Serbian.game_code(), Some("sr_sp"));
        assert_eq!(Language::Chuvash.game_code(), Some("cv_cu"));
        assert_eq!(Language::Urdu.game_code(), None);
        assert_eq!(Language::Zulu.game_code(), None);
    }

    #[test]
    fn options_txt_follows_the_chosen_language_without_losing_other_lines() {
        let root = std::env::temp_dir().join(format!("meridian-lang-opt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("profiles/modded")).unwrap();
        fs::create_dir_all(root.join("profiles/custom")).unwrap();
        fs::write(
            root.join("profiles/modded/options.txt"),
            "version:4892\nfov:0.5\nlang:en_us\n",
        )
        .unwrap();
        fs::write(
            root.join("profiles/custom/options.txt"),
            "version:4892\nlang:ja_jp\n",
        )
        .unwrap();

        save_language(&root, Language::French).unwrap();
        assert_eq!(saved_language(&root), Language::French);
        assert_eq!(
            fs::read_to_string(root.join("options.txt")).unwrap(),
            "version:4892\nlang:fr_fr\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("profiles/modded/options.txt")).unwrap(),
            "version:4892\nfov:0.5\nlang:fr_fr\n"
        );
        // A different game language the player already chose in Minecraft is left alone.
        assert_eq!(
            fs::read_to_string(root.join("profiles/custom/options.txt")).unwrap(),
            "version:4892\nlang:ja_jp\n"
        );
        // Switching again from French to German updates profiles that followed French.
        save_language(&root, Language::German).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("profiles/modded/options.txt")).unwrap(),
            "version:4892\nfov:0.5\nlang:de_de\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("profiles/custom/options.txt")).unwrap(),
            "version:4892\nlang:ja_jp\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn download_rejects_bad_checksum_and_falls_back_to_english() {
        let root = std::env::temp_dir().join(format!("meridian-lang-dl-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut req = [0u8; 512];
                let _ = stream.read(&mut req);
                let body = b"{\"Settings\":\"Tampered\"}";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.write_all(body);
            }
        });

        let err = fetch_from(&format!("http://{addr}"), &root, Language::Polish).unwrap_err();
        handle.join().unwrap();
        assert!(err.contains("checksum"), "{err}");
        assert!(!is_cached(&root, Language::Polish));
        assert_eq!(tr_in(Language::Polish, "Settings"), "Settings");
        let _ = fs::remove_dir_all(root);
    }
}
