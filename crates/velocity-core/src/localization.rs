//! Localization and internationalization (i18n) system.
//!
//! Provides a string table with language fallback for all UI text shown
//! during installation. Supports:
//! - Built-in English strings as the base language
//! - Override strings from velocity.toml
//! - Per-language string tables
//! - Runtime language switching

use std::collections::HashMap;
use velocity_config::{LanguageEntry, LocalizationConfig};

/// The localization engine that resolves UI strings.
#[derive(Debug, Clone)]
pub struct Localizer {
    /// Current language code
    current_language: String,
    /// Built-in English defaults (always present as fallback)
    defaults: HashMap<&'static str, &'static str>,
    /// User-provided string overrides for the current language
    overrides: HashMap<String, String>,
    /// All available languages
    languages: Vec<LanguageEntry>,
}

impl Localizer {
    /// Create a new localizer from the manifest's localization config.
    pub fn new(config: &LocalizationConfig) -> Self {
        let mut defaults = HashMap::new();

        // ── Built-in English strings ──────────────────────────────────────
        // Wizard pages
        defaults.insert("wizard_title", "Installation Wizard");
        defaults.insert("wizard_welcome_title", "Welcome");
        defaults.insert(
            "wizard_welcome_subtitle",
            "Welcome to the {app_name} Setup Wizard",
        );
        defaults.insert("wizard_welcome_body", "This will install {app_name} {version} on your computer.\n\nClick Next to continue, or Cancel to exit the Setup Wizard.");
        defaults.insert("wizard_license_title", "License Agreement");
        defaults.insert(
            "wizard_license_subtitle",
            "Please read the license agreement carefully",
        );
        defaults.insert(
            "wizard_license_accept",
            "I accept the terms of the license agreement",
        );
        defaults.insert(
            "wizard_license_decline",
            "I do not accept the terms of the license agreement",
        );
        defaults.insert("wizard_select_dir_title", "Select Installation Folder");
        defaults.insert(
            "wizard_select_dir_subtitle",
            "Choose where to install {app_name}",
        );
        defaults.insert("wizard_select_dir_label", "Installation folder:");
        defaults.insert("wizard_select_dir_browse", "Browse...");
        defaults.insert("wizard_components_title", "Select Components");
        defaults.insert(
            "wizard_components_subtitle",
            "Choose which features to install",
        );
        defaults.insert("wizard_install_title", "Installing");
        defaults.insert("wizard_install_subtitle", "Setting up {app_name}");
        defaults.insert("wizard_finish_title", "Installation Complete");
        defaults.insert("wizard_finish_subtitle", "{app_name} has been installed");
        defaults.insert(
            "wizard_finish_body",
            "Click Finish to exit the Setup Wizard.",
        );
        defaults.insert("wizard_finish_launch", "Launch {app_name}");

        // Buttons
        defaults.insert("btn_next", "&Next >");
        defaults.insert("btn_back", "< &Back");
        defaults.insert("btn_install", "&Install");
        defaults.insert("btn_finish", "&Finish");
        defaults.insert("btn_cancel", "Cancel");
        defaults.insert("btn_browse", "&Browse...");
        defaults.insert("btn_yes", "&Yes");
        defaults.insert("btn_no", "&No");
        defaults.insert("btn_ok", "OK");
        defaults.insert("btn_close", "Close");
        defaults.insert("btn_retry", "&Retry");

        // Messages
        defaults.insert(
            "msg_confirm_cancel",
            "Are you sure you want to cancel the installation?",
        );
        defaults.insert(
            "msg_confirm_uninstall",
            "Are you sure you want to uninstall {app_name}?",
        );
        defaults.insert(
            "msg_install_complete",
            "{app_name} has been successfully installed.",
        );
        defaults.insert(
            "msg_uninstall_complete",
            "{app_name} has been successfully removed.",
        );
        defaults.insert("msg_install_failed", "Installation failed. Error: {error}");
        defaults.insert(
            "msg_disk_space",
            "Not enough disk space. Required: {required}, Available: {available}",
        );
        defaults.insert(
            "msg_app_running",
            "{app_name} is currently running. Please close it before continuing.",
        );
        defaults.insert(
            "msg_elevation_required",
            "Administrator privileges are required to install this application.",
        );
        defaults.insert("msg_extracting", "Extracting files...");
        defaults.insert("msg_creating_shortcuts", "Creating shortcuts...");
        defaults.insert("msg_writing_registry", "Writing registry entries...");
        defaults.insert("msg_installing_deps", "Installing dependencies...");
        defaults.insert("msg_generating_uninstaller", "Generating uninstaller...");
        defaults.insert("msg_rolling_back", "Rolling back changes...");

        // Uninstall
        defaults.insert("uninstall_title", "Uninstall {app_name}");
        defaults.insert("uninstall_progress", "Removing {app_name}...");

        // Progress
        defaults.insert("progress_file", "File {current} of {total}: {name}");
        defaults.insert("progress_percent", "{percent}% complete");
        defaults.insert("progress_eta", "Estimated time remaining: {time}");

        // Component selection
        defaults.insert("comp_select_all", "Select All");
        defaults.insert("comp_deselect_all", "Deselect All");
        defaults.insert("comp_space_required", "Space required: {size}");
        defaults.insert("comp_space_available", "Space available: {size}");

        // Apply user overrides from the manifest
        let mut overrides = HashMap::new();
        for (key, value) in &config.strings {
            overrides.insert(key.clone(), value.clone());
        }

        // If the current language has overrides, apply them
        if let Some(lang) = config
            .languages
            .iter()
            .find(|l| l.code == config.default_language)
        {
            for (key, value) in &lang.strings {
                overrides.insert(key.clone(), value.clone());
            }
        }

        Self {
            current_language: config.default_language.clone(),
            defaults,
            overrides,
            languages: config.languages.clone(),
        }
    }

    /// Create a localizer with just defaults (no config).
    pub fn defaults_only() -> Self {
        Self::new(&LocalizationConfig::default())
    }

    /// Resolve a string by key, substituting variables.
    ///
    /// Variables use the format `{variable_name}` and are replaced
    /// with values from the `vars` slice (key-value pairs).
    pub fn get(&self, key: &str, vars: &[(&str, &str)]) -> String {
        // Look up: overrides first, then defaults
        let template = self
            .overrides
            .get(key)
            .map(|s| s.as_str())
            .or_else(|| self.defaults.get(key).copied())
            .unwrap_or(key);

        // Substitute variables
        let mut result = template.to_string();
        for (var_name, var_value) in vars {
            result = result.replace(&format!("{{{}}}", var_name), var_value);
        }
        result
    }

    /// Resolve a string with no variable substitution.
    pub fn get_simple(&self, key: &str) -> String {
        self.get(key, &[])
    }

    /// Switch to a different language.
    pub fn set_language(&mut self, code: &str) {
        self.current_language = code.to_string();

        // Apply language-specific overrides
        if let Some(lang) = self.languages.iter().find(|l| l.code == code) {
            for (key, value) in &lang.strings {
                self.overrides.insert(key.clone(), value.clone());
            }
        }
    }

    /// Get the current language code.
    pub fn current_language(&self) -> &str {
        &self.current_language
    }

    /// Get available languages.
    pub fn available_languages(&self) -> &[LanguageEntry] {
        &self.languages
    }

    /// Check if a key exists.
    pub fn has_key(&self, key: &str) -> bool {
        self.overrides.contains_key(key) || self.defaults.contains_key(key)
    }

    /// Get all known keys.
    pub fn all_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.defaults.keys().copied().collect();
        for key in self.overrides.keys() {
            if !self.defaults.contains_key(key.as_str()) {
                keys.push(key.as_str());
            }
        }
        keys.sort();
        keys
    }
}

/// Built-in language packs for common languages.
/// These provide basic translations for UI elements.
pub fn builtin_language_packs() -> Vec<LanguageEntry> {
    vec![
        LanguageEntry {
            code: "de".to_string(),
            name: "Deutsch".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Weiter >".into());
                m.insert("btn_back".into(), "< &Zurueck".into());
                m.insert("btn_install".into(), "&Installieren".into());
                m.insert("btn_finish".into(), "&Fertigstellen".into());
                m.insert("btn_cancel".into(), "Abbrechen".into());
                m.insert("btn_browse".into(), "&Durchsuchen...".into());
                m.insert("btn_yes".into(), "&Ja".into());
                m.insert("btn_no".into(), "&Nein".into());
                m.insert("wizard_welcome_title".into(), "Willkommen".into());
                m.insert("wizard_install_title".into(), "Installation".into());
                m.insert(
                    "wizard_finish_title".into(),
                    "Installation abgeschlossen".into(),
                );
                m.insert(
                    "msg_confirm_cancel".into(),
                    "Moechten Sie die Installation wirklich abbrechen?".into(),
                );
                m.insert(
                    "msg_extracting".into(),
                    "Dateien werden extrahiert...".into(),
                );
                m.insert(
                    "msg_install_complete".into(),
                    "{app_name} wurde erfolgreich installiert.".into(),
                );
                m
            },
        },
        LanguageEntry {
            code: "fr".to_string(),
            name: "Francais".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Suivant >".into());
                m.insert("btn_back".into(), "< &Precedent".into());
                m.insert("btn_install".into(), "&Installer".into());
                m.insert("btn_finish".into(), "&Terminer".into());
                m.insert("btn_cancel".into(), "Annuler".into());
                m.insert("btn_browse".into(), "&Parcourir...".into());
                m.insert("btn_yes".into(), "&Oui".into());
                m.insert("btn_no".into(), "&Non".into());
                m.insert("wizard_welcome_title".into(), "Bienvenue".into());
                m.insert("wizard_install_title".into(), "Installation".into());
                m.insert("wizard_finish_title".into(), "Installation terminee".into());
                m.insert(
                    "msg_confirm_cancel".into(),
                    "Voulez-vous vraiment annuler l'installation?".into(),
                );
                m.insert("msg_extracting".into(), "Extraction des fichiers...".into());
                m.insert(
                    "msg_install_complete".into(),
                    "{app_name} a ete installe avec succes.".into(),
                );
                m
            },
        },
        LanguageEntry {
            code: "es".to_string(),
            name: "Espanol".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Siguiente >".into());
                m.insert("btn_back".into(), "< &Atras".into());
                m.insert("btn_install".into(), "&Instalar".into());
                m.insert("btn_finish".into(), "&Finalizar".into());
                m.insert("btn_cancel".into(), "Cancelar".into());
                m.insert("btn_browse".into(), "&Examinar...".into());
                m.insert("btn_yes".into(), "&Si".into());
                m.insert("btn_no".into(), "&No".into());
                m.insert("wizard_welcome_title".into(), "Bienvenido".into());
                m.insert("wizard_install_title".into(), "Instalacion".into());
                m.insert(
                    "wizard_finish_title".into(),
                    "Instalacion completada".into(),
                );
                m.insert(
                    "msg_confirm_cancel".into(),
                    "Esta seguro de que desea cancelar la instalacion?".into(),
                );
                m.insert("msg_extracting".into(), "Extrayendo archivos...".into());
                m.insert(
                    "msg_install_complete".into(),
                    "{app_name} se ha instalado correctamente.".into(),
                );
                m
            },
        },
        LanguageEntry {
            code: "ja".to_string(),
            name: "日本語".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "次へ(&N) >".into());
                m.insert("btn_back".into(), "< 戻る(&B)".into());
                m.insert("btn_install".into(), "インストール(&I)".into());
                m.insert("btn_finish".into(), "完了(&F)".into());
                m.insert("btn_cancel".into(), "キャンセル".into());
                m.insert("btn_browse".into(), "参照(&B)...".into());
                m.insert("wizard_welcome_title".into(), "ようこそ".into());
                m.insert("wizard_install_title".into(), "インストール中".into());
                m.insert("wizard_finish_title".into(), "インストール完了".into());
                m.insert("msg_extracting".into(), "ファイルを展開中...".into());
                m
            },
        },
        LanguageEntry {
            code: "zh".to_string(),
            name: "中文".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "下一步(&N) >".into());
                m.insert("btn_back".into(), "< 上一步(&B)".into());
                m.insert("btn_install".into(), "安装(&I)".into());
                m.insert("btn_finish".into(), "完成(&F)".into());
                m.insert("btn_cancel".into(), "取消".into());
                m.insert("btn_browse".into(), "浏览(&B)...".into());
                m.insert("wizard_welcome_title".into(), "欢迎".into());
                m.insert("wizard_install_title".into(), "正在安装".into());
                m.insert("wizard_finish_title".into(), "安装完成".into());
                m.insert("msg_extracting".into(), "正在解压文件...".into());
                m
            },
        },
        LanguageEntry {
            code: "pt".to_string(),
            name: "Portugues".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Proximo >".into());
                m.insert("btn_back".into(), "< &Voltar".into());
                m.insert("btn_install".into(), "&Instalar".into());
                m.insert("btn_finish".into(), "&Concluir".into());
                m.insert("btn_cancel".into(), "Cancelar".into());
                m.insert("btn_browse".into(), "&Procurar...".into());
                m.insert("wizard_welcome_title".into(), "Bem-vindo".into());
                m.insert("wizard_install_title".into(), "Instalando".into());
                m.insert("wizard_finish_title".into(), "Instalacao concluida".into());
                m.insert("msg_extracting".into(), "Extraindo arquivos...".into());
                m
            },
        },
        LanguageEntry {
            code: "ko".to_string(),
            name: "한국어".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "다음(&N) >".into());
                m.insert("btn_back".into(), "< 뒤로(&B)".into());
                m.insert("btn_install".into(), "설치(&I)".into());
                m.insert("btn_finish".into(), "완료(&F)".into());
                m.insert("btn_cancel".into(), "취소".into());
                m.insert("wizard_welcome_title".into(), "환영".into());
                m.insert("wizard_install_title".into(), "설치 중".into());
                m.insert("wizard_finish_title".into(), "설치 완료".into());
                m.insert("msg_extracting".into(), "파일 압축 해제 중...".into());
                m
            },
        },
        LanguageEntry {
            code: "it".to_string(),
            name: "Italiano".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Avanti >".into());
                m.insert("btn_back".into(), "< &Indietro".into());
                m.insert("btn_install".into(), "&Installa".into());
                m.insert("btn_finish".into(), "&Fine".into());
                m.insert("btn_cancel".into(), "Annulla".into());
                m.insert("btn_browse".into(), "&Sfoglia...".into());
                m.insert("btn_yes".into(), "&Si".into());
                m.insert("btn_no".into(), "&No".into());
                m.insert("wizard_welcome_title".into(), "Benvenuto".into());
                m.insert("wizard_install_title".into(), "Installazione".into());
                m.insert(
                    "wizard_finish_title".into(),
                    "Installazione completata".into(),
                );
                m.insert(
                    "msg_confirm_cancel".into(),
                    "Si desidera annullare l'installazione?".into(),
                );
                m.insert("msg_extracting".into(), "Estrazione file...".into());
                m.insert(
                    "msg_install_complete".into(),
                    "{app_name} e stato installato correttamente.".into(),
                );
                m
            },
        },
        LanguageEntry {
            code: "nl".to_string(),
            name: "Nederlands".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Volgende >".into());
                m.insert("btn_back".into(), "< &Vorige".into());
                m.insert("btn_install".into(), "&Installeren".into());
                m.insert("btn_finish".into(), "&Voltooien".into());
                m.insert("btn_cancel".into(), "Annuleren".into());
                m.insert("btn_browse".into(), "&Bladeren...".into());
                m.insert("wizard_welcome_title".into(), "Welkom".into());
                m.insert("wizard_install_title".into(), "Installatie".into());
                m.insert("wizard_finish_title".into(), "Installatie voltooid".into());
                m.insert("msg_extracting".into(), "Bestanden uitpakken...".into());
                m
            },
        },
        LanguageEntry {
            code: "ru".to_string(),
            name: "Русский".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Далее >".into());
                m.insert("btn_back".into(), "< &Назад".into());
                m.insert("btn_install".into(), "&Установить".into());
                m.insert("btn_finish".into(), "&Готово".into());
                m.insert("btn_cancel".into(), "Отмена".into());
                m.insert("btn_browse".into(), "&Обзор...".into());
                m.insert("btn_yes".into(), "&Да".into());
                m.insert("btn_no".into(), "&Нет".into());
                m.insert("wizard_welcome_title".into(), "Добро пожаловать".into());
                m.insert("wizard_install_title".into(), "Установка".into());
                m.insert("wizard_finish_title".into(), "Установка завершена".into());
                m.insert(
                    "msg_confirm_cancel".into(),
                    "Вы действительно хотите отменить установку?".into(),
                );
                m.insert("msg_extracting".into(), "Распаковка файлов...".into());
                m.insert(
                    "msg_install_complete".into(),
                    "{app_name} успешно установлен.".into(),
                );
                m
            },
        },
        LanguageEntry {
            code: "pl".to_string(),
            name: "Polski".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Dalej >".into());
                m.insert("btn_back".into(), "< &Wstecz".into());
                m.insert("btn_install".into(), "&Zainstaluj".into());
                m.insert("btn_finish".into(), "&Zakoncz".into());
                m.insert("btn_cancel".into(), "Anuluj".into());
                m.insert("btn_browse".into(), "&Przegladaj...".into());
                m.insert("wizard_welcome_title".into(), "Witamy".into());
                m.insert("wizard_install_title".into(), "Instalacja".into());
                m.insert("wizard_finish_title".into(), "Instalacja zakonczona".into());
                m.insert("msg_extracting".into(), "Rozpakiwanie plikow...".into());
                m
            },
        },
        LanguageEntry {
            code: "sv".to_string(),
            name: "Svenska".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Nasta >".into());
                m.insert("btn_back".into(), "< &Tillbaka".into());
                m.insert("btn_install".into(), "&Installera".into());
                m.insert("btn_finish".into(), "&Slutför".into());
                m.insert("btn_cancel".into(), "Avbryt".into());
                m.insert("btn_browse".into(), "&Blaeddra...".into());
                m.insert("wizard_welcome_title".into(), "Valkommen".into());
                m.insert("wizard_install_title".into(), "Installation".into());
                m.insert(
                    "wizard_finish_title".into(),
                    "Installationen ar klar".into(),
                );
                m.insert("msg_extracting".into(), "Packar upp filer...".into());
                m
            },
        },
        LanguageEntry {
            code: "tr".to_string(),
            name: "Turkce".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Ileri >".into());
                m.insert("btn_back".into(), "< &Geri".into());
                m.insert("btn_install".into(), "&Yukle".into());
                m.insert("btn_finish".into(), "&Bitir".into());
                m.insert("btn_cancel".into(), "Iptal".into());
                m.insert("btn_browse".into(), "&Gozat...".into());
                m.insert("wizard_welcome_title".into(), "Hos Geldiniz".into());
                m.insert("wizard_install_title".into(), "Yukleniyor".into());
                m.insert("wizard_finish_title".into(), "Yukleme Tamamlandi".into());
                m.insert("msg_extracting".into(), "Dosyalar cikariliyor...".into());
                m
            },
        },
        LanguageEntry {
            code: "ar".to_string(),
            name: "العربية".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "> التالي(&N)".into());
                m.insert("btn_back".into(), "(&B) السابق <".into());
                m.insert("btn_install".into(), "(&I) تثبيت".into());
                m.insert("btn_finish".into(), "(&F) انهاء".into());
                m.insert("btn_cancel".into(), "الغاء".into());
                m.insert("btn_browse".into(), "...استعراض(&B)".into());
                m.insert("wizard_welcome_title".into(), "مرحبا".into());
                m.insert("wizard_install_title".into(), "جارٍ التثبيت".into());
                m.insert("wizard_finish_title".into(), "اكتمل التثبيت".into());
                m.insert("msg_extracting".into(), "...جارٍ استخراج الملفات".into());
                m
            },
        },
        LanguageEntry {
            code: "cs".to_string(),
            name: "Cesky".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Dale >".into());
                m.insert("btn_back".into(), "< &Zpet".into());
                m.insert("btn_install".into(), "&Instalovat".into());
                m.insert("btn_finish".into(), "&Dokonciti".into());
                m.insert("btn_cancel".into(), "Zrusit".into());
                m.insert("btn_browse".into(), "&Prochazet...".into());
                m.insert("wizard_welcome_title".into(), "Vitejte".into());
                m.insert("wizard_install_title".into(), "Instalace".into());
                m.insert("wizard_finish_title".into(), "Instalace dokoncena".into());
                m.insert("msg_extracting".into(), "Rozbalovani souboru...".into());
                m
            },
        },
        LanguageEntry {
            code: "hu".to_string(),
            name: "Magyar".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Tovabb >".into());
                m.insert("btn_back".into(), "< &Vissza".into());
                m.insert("btn_install".into(), "&Telepites".into());
                m.insert("btn_finish".into(), "&Befejezes".into());
                m.insert("btn_cancel".into(), "Megse".into());
                m.insert("wizard_welcome_title".into(), "Udvozoljuk".into());
                m.insert("wizard_install_title".into(), "Telepites".into());
                m.insert(
                    "wizard_finish_title".into(),
                    "A telepites befejezodott".into(),
                );
                m.insert("msg_extracting".into(), "Fajlok kibontasa...".into());
                m
            },
        },
        LanguageEntry {
            code: "ro".to_string(),
            name: "Romana".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Urmatorul >".into());
                m.insert("btn_back".into(), "< &Inapoi".into());
                m.insert("btn_install".into(), "&Instalare".into());
                m.insert("btn_finish".into(), "&Finalizare".into());
                m.insert("btn_cancel".into(), "Anulare".into());
                m.insert("btn_browse".into(), "&Rasfoire...".into());
                m.insert("wizard_welcome_title".into(), "Bun venit".into());
                m.insert("wizard_install_title".into(), "Instalare".into());
                m.insert("wizard_finish_title".into(), "Instalare completa".into());
                m.insert("msg_extracting".into(), "Extragere fisiere...".into());
                m
            },
        },
        LanguageEntry {
            code: "uk".to_string(),
            name: "Українська".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Дали >".into());
                m.insert("btn_back".into(), "< &Назад".into());
                m.insert("btn_install".into(), "&Встановити".into());
                m.insert("btn_finish".into(), "&Готово".into());
                m.insert("btn_cancel".into(), "Скасувати".into());
                m.insert("btn_browse".into(), "&Огляд...".into());
                m.insert("wizard_welcome_title".into(), "Ласкаво просимо".into());
                m.insert("wizard_install_title".into(), "Встановлення".into());
                m.insert(
                    "wizard_finish_title".into(),
                    "Встановлення завершено".into(),
                );
                m.insert("msg_extracting".into(), "Розпакування файлів...".into());
                m
            },
        },
        LanguageEntry {
            code: "vi".to_string(),
            name: "Tieng Viet".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "&Tiep >".into());
                m.insert("btn_back".into(), "< &Quay lai".into());
                m.insert("btn_install".into(), "&Cai dat".into());
                m.insert("btn_finish".into(), "&Hoan tat".into());
                m.insert("btn_cancel".into(), "Huy".into());
                m.insert("wizard_welcome_title".into(), "Chao mung".into());
                m.insert("wizard_install_title".into(), "Dang cai dat".into());
                m.insert("wizard_finish_title".into(), "Cai dat hoan tat".into());
                m.insert("msg_extracting".into(), "Dang giai nen tap tin...".into());
                m
            },
        },
        LanguageEntry {
            code: "th".to_string(),
            name: "ภาษาไทย".to_string(),
            strings: {
                let mut m = HashMap::new();
                m.insert("btn_next".into(), "> ถัดไป(&N)".into());
                m.insert("btn_back".into(), "(&B) ย้อนกลับ <".into());
                m.insert("btn_install".into(), "(&I) ติดตั้ง".into());
                m.insert("btn_finish".into(), "(&F) เสร็จสิ้น".into());
                m.insert("btn_cancel".into(), "ยกเลิก".into());
                m.insert("wizard_welcome_title".into(), "ยินดีต้อนรับ".into());
                m.insert("wizard_install_title".into(), "กำลังติดตั้ง".into());
                m.insert("wizard_finish_title".into(), "ติดตั้งเสร็จสมบูรณ์".into());
                m.insert("msg_extracting".into(), "กำลังแตกไฟล์...".into());
                m
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_strings() {
        let loc = Localizer::defaults_only();
        assert_eq!(loc.get_simple("btn_next"), "&Next >");
        assert_eq!(loc.get_simple("btn_cancel"), "Cancel");
        assert_eq!(loc.get_simple("btn_finish"), "&Finish");
    }

    #[test]
    fn test_variable_substitution() {
        let loc = Localizer::defaults_only();
        let result = loc.get("msg_install_complete", &[("app_name", "TestApp")]);
        assert_eq!(result, "TestApp has been successfully installed.");
    }

    #[test]
    fn test_multiple_variables() {
        let loc = Localizer::defaults_only();
        let result = loc.get(
            "msg_disk_space",
            &[("required", "500 MB"), ("available", "200 MB")],
        );
        assert!(result.contains("500 MB"));
        assert!(result.contains("200 MB"));
    }

    #[test]
    fn test_unknown_key_returns_key() {
        let loc = Localizer::defaults_only();
        assert_eq!(loc.get_simple("nonexistent_key"), "nonexistent_key");
    }

    #[test]
    fn test_override_strings() {
        let mut config = LocalizationConfig::default();
        config
            .strings
            .insert("btn_next".to_string(), "Continue >>".to_string());
        let loc = Localizer::new(&config);
        assert_eq!(loc.get_simple("btn_next"), "Continue >>");
    }

    #[test]
    fn test_language_switch() {
        let config = LocalizationConfig {
            default_language: "en".to_string(),
            languages: builtin_language_packs(),
            strings: HashMap::new(),
        };
        let mut loc = Localizer::new(&config);
        assert_eq!(loc.get_simple("btn_next"), "&Next >");

        loc.set_language("de");
        assert_eq!(loc.get_simple("btn_next"), "&Weiter >");

        loc.set_language("fr");
        assert_eq!(loc.get_simple("btn_next"), "&Suivant >");
    }

    #[test]
    fn test_has_key() {
        let loc = Localizer::defaults_only();
        assert!(loc.has_key("btn_next"));
        assert!(!loc.has_key("nonexistent"));
    }

    #[test]
    fn test_builtin_language_packs() {
        let packs = builtin_language_packs();
        assert!(packs.len() >= 7);
        assert!(packs.iter().any(|p| p.code == "de"));
        assert!(packs.iter().any(|p| p.code == "fr"));
        assert!(packs.iter().any(|p| p.code == "ja"));
        assert!(packs.iter().any(|p| p.code == "zh"));
    }

    #[test]
    fn test_all_keys() {
        let loc = Localizer::defaults_only();
        let keys = loc.all_keys();
        assert!(keys.len() > 30);
        assert!(keys.contains(&"btn_next"));
        assert!(keys.contains(&"wizard_welcome_title"));
    }
}
