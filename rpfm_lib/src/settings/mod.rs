//---------------------------------------------------------------------------//
// Copyright (c) 2017-2020 Ismael Gutiérrez González. All rights reserved.
//
// This file is part of the Rusted PackFile Manager (RPFM) project,
// which can be found here: https://github.com/Frodo45127/rpfm.
//
// This file is licensed under the MIT license, which can be found here:
// https://github.com/Frodo45127/rpfm/blob/master/LICENSE.
//---------------------------------------------------------------------------//

/*!
Module with all the code to deal with the settings used to configure this lib.

This module contains all the code related with the settings used by the lib to work. These
settings are saved in the config folder, in a file called `settings.ron`, in case you want
to change them manually.
!*/

use ron::de::{from_reader, from_str};
use ron::ser::{to_string_pretty, PrettyConfig};
use serde_derive::{Serialize, Deserialize};

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};

use rpfm_error::Result;

use crate::games::*;
use crate::SUPPORTED_GAMES;
use crate::config::get_config_path;
use crate::updater::STABLE;

/// Name of the settings file.
const SETTINGS_FILE: &str = "settings.ron";

/// Key of the 7Zip path in the settings";
pub const ZIP_PATH: &str = "7zip_path";

/// Key of the MyMod path in the settings";
pub const MYMOD_BASE_PATH: &str = "mymods_base_path";

/// This struct hold every setting of the lib and of RPFM_UI/CLI.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    pub paths: BTreeMap<String, Option<PathBuf>>,
    pub settings_string: BTreeMap<String, String>,
    pub settings_bool: BTreeMap<String, bool>,
}

/// Implementation of `Settings`.
impl Settings {

    /// This function creates a new default `Settings`.
    ///
    /// Should be run if no settings file has been found at the start of any program using this lib.
    pub fn new() -> Self {
        let mut paths = BTreeMap::new();
        let mut settings_string = BTreeMap::new();
        let mut settings_bool = BTreeMap::new();
        paths.insert(MYMOD_BASE_PATH.to_owned(), None);
        paths.insert(ZIP_PATH.to_owned(), None);
        for (folder_name, _) in SUPPORTED_GAMES.iter() {
            paths.insert((*folder_name).to_string(), None);
        }

        // General Settings.
        settings_string.insert("default_game".to_owned(), KEY_THREE_KINGDOMS.to_owned());
        settings_string.insert("language".to_owned(), "English_en".to_owned());
        settings_string.insert("update_channel".to_owned(), STABLE.to_owned());
        settings_string.insert("autosave_amount".to_owned(), "10".to_owned());
        settings_string.insert("autosave_interval".to_owned(), "5".to_owned());
        settings_string.insert("font_name".to_owned(), "".to_owned());
        settings_string.insert("font_size".to_owned(), "".to_owned());
        settings_string.insert("recent_files".to_owned(), "[]".to_owned());

        // UI Settings.
        settings_bool.insert("start_maximized".to_owned(), false);
        settings_bool.insert("use_dark_theme".to_owned(), false);
        settings_bool.insert("hide_background_icon".to_owned(), false);
        settings_bool.insert("allow_editing_of_ca_packfiles".to_owned(), false);
        settings_bool.insert("check_updates_on_start".to_owned(), true);
        settings_bool.insert("check_schema_updates_on_start".to_owned(), true);
        settings_bool.insert("check_template_updates_on_start".to_owned(), true);
        settings_bool.insert("use_lazy_loading".to_owned(), true);
        settings_bool.insert("optimize_not_renamed_packedfiles".to_owned(), false);
        settings_bool.insert("disable_uuid_regeneration_on_db_tables".to_owned(), false);
        settings_bool.insert("packfile_treeview_resize_to_fit".to_owned(), false);
        settings_bool.insert("expand_treeview_when_adding_items".to_owned(), true);

        // Table Settings.
        settings_bool.insert("adjust_columns_to_content".to_owned(), true);
        settings_bool.insert("extend_last_column_on_tables".to_owned(), true);
        settings_bool.insert("disable_combos_on_tables".to_owned(), false);
        settings_bool.insert("tight_table_mode".to_owned(), false);
        settings_bool.insert("table_resize_on_edit".to_owned(), false);
        settings_bool.insert("tables_use_old_column_order".to_owned(), false);

        // Debug Settings.
        settings_bool.insert("check_for_missing_table_definitions".to_owned(), false);
        settings_bool.insert("enable_debug_menu".to_owned(), false);
        settings_bool.insert("spoof_ca_authoring_tool".to_owned(), false);

        // Diagnostics Settings
        settings_bool.insert("enable_diagnostics_tool".to_owned(), true);
        settings_bool.insert("diagnostics_trigger_on_open".to_owned(), true);
        settings_bool.insert("diagnostics_trigger_on_table_edit".to_owned(), true);

        Self {
            paths,
            settings_string,
            settings_bool,
        }
    }

    /// This function tries to load the `settings.ron` from disk, if exist, and return it.
    pub fn load(file_path: Option<&str>) -> Result<Self> {
        let file_path = if let Some(file_path) = file_path { PathBuf::from(file_path) } else { get_config_path()?.join(SETTINGS_FILE) };
        let file = BufReader::new(File::open(file_path)?);
        let mut settings: Self = from_reader(file)?;

        // Add/Remove settings missing/no-longer-needed for keeping it update friendly. First, remove the outdated ones, then add the new ones.
        let defaults = Self::new();
        {
            let mut keys_to_delete = vec![];
            for (key, _) in settings.paths.clone() { if defaults.paths.get(&*key).is_none() { keys_to_delete.push(key); } }
            for key in &keys_to_delete { settings.paths.remove(key); }

            let mut keys_to_delete = vec![];
            for (key, _) in settings.settings_string.clone() { if defaults.settings_string.get(&*key).is_none() { keys_to_delete.push(key); } }
            for key in &keys_to_delete { settings.settings_string.remove(key); }

            let mut keys_to_delete = vec![];
            for (key, _) in settings.settings_bool.clone() { if defaults.settings_bool.get(&*key).is_none() { keys_to_delete.push(key); } }
            for key in &keys_to_delete { settings.settings_bool.remove(key); }
        }

        {
            for (key, value) in defaults.paths { if settings.paths.get(&*key).is_none() { settings.paths.insert(key, value);  } }
            for (key, value) in defaults.settings_string { if settings.settings_string.get(&*key).is_none() { settings.settings_string.insert(key, value);  } }
            for (key, value) in defaults.settings_bool { if settings.settings_bool.get(&*key).is_none() { settings.settings_bool.insert(key, value);  } }
        }

        Ok(settings)
    }

    /// This function tries to save the provided `Settings` to disk.
    pub fn save(&self) -> Result<()> {
        let file_path = get_config_path()?.join(SETTINGS_FILE);
        let mut file = BufWriter::new(File::create(file_path)?);
        let config = PrettyConfig::default();
        file.write_all(to_string_pretty(&self, config)?.as_bytes())?;
        Ok(())
    }

    pub fn get_recent_files(&self) -> Vec<String> {
        from_str(self.settings_string.get("recent_files").unwrap()).unwrap()
    }

    pub fn set_recent_files(&mut self, recent_files: &[String]) {
        let config = PrettyConfig::default();
        *self.settings_string.get_mut("recent_files").unwrap() = to_string_pretty(&recent_files, config).unwrap();
        let _ = self.save();
    }

    pub fn update_recent_files(&mut self, new_path: &str) {
        *self = Self::load(None).unwrap_or_else(|_|Settings::new());
        if let Some(recent_files) = self.settings_string.get("recent_files") {
            let mut recent_files: Vec<String> = from_str(recent_files).unwrap();

            if let Some(index) = recent_files.iter().position(|x| x == new_path) {
                recent_files.remove(index);
            }

            recent_files.reverse();
            recent_files.push(new_path.to_owned());
            recent_files.reverse();

            // Limit it to 10 Packfiles.
            recent_files.truncate(10);

            let config = PrettyConfig::default();
            *self.settings_string.get_mut("recent_files").unwrap() = to_string_pretty(&recent_files, config).unwrap();
            let _ = self.save();
        }
    }
}

