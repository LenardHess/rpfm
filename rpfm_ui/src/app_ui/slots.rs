//---------------------------------------------------------------------------//
// Copyright (c) 2017-2019 Ismael Gutiérrez González. All rights reserved.
//
// This file is part of the Rusted PackFile Manager (RPFM) project,
// which can be found here: https://github.com/Frodo45127/rpfm.
//
// This file is licensed under the MIT license, which can be found here:
// https://github.com/Frodo45127/rpfm/blob/master/LICENSE.
//---------------------------------------------------------------------------//

/*!
Module with all the code related to the main `AppUISlot`.
!*/

use qt_widgets::action::Action;
use qt_widgets::completer::Completer;
use qt_widgets::file_dialog::{FileDialog, FileMode, Option::ShowDirsOnly};
use qt_widgets::message_box::MessageBox;
use qt_widgets::widget::Widget;

use qt_gui::desktop_services::DesktopServices;

use qt_core::qt::FocusReason;
use qt_core::slots::{SlotBool, SlotNoArgs, SlotStringRef};

use std::path::PathBuf;

use rpfm_error::ErrorKind;
use rpfm_lib::common::*;
use rpfm_lib::DOCS_BASE_URL;
use rpfm_lib::GAME_SELECTED;
use rpfm_lib::packfile::PFHFileType;
use rpfm_lib::PATREON_URL;
use rpfm_lib::SETTINGS;
use rpfm_lib::SUPPORTED_GAMES;


use crate::QString;
use crate::app_ui::AppUI;
use crate::CENTRAL_COMMAND;
use crate::command_palette;
use crate::communications::{THREADS_COMMUNICATION_ERROR, Command, Response};
use crate::global_search_ui::GlobalSearchUI;
use crate::pack_tree::{PackTree, TreeViewOperation};
use crate::packfile_contents_ui::PackFileContentsUI;
use crate::pack_tree::TreePathType;
use crate::settings_ui::SettingsUI;
use crate::ui::GameSelectedIcons;

use crate::UI_STATE;
use crate::utils::show_dialog;

//-------------------------------------------------------------------------------//
//                              Enums & Structs
//-------------------------------------------------------------------------------//

/// This struct contains all the slots we need to respond to signals of EVERY widget/action created at the start of the program.
///
/// This means everything you can do with the stuff you have in the `AppUI` goes here.
pub struct AppUISlots {

	//-----------------------------------------------//
    // Command Palette slots.
    //-----------------------------------------------//
    pub command_palette_show: SlotNoArgs<'static>,
    pub command_palette_hide: SlotNoArgs<'static>,
    pub command_palette_trigger: SlotStringRef<'static>,

    //-----------------------------------------------//
    // `PackFile` menu slots.
    //-----------------------------------------------//
    pub packfile_new_packfile: SlotBool<'static>,
    pub packfile_open_packfile: SlotBool<'static>,
    pub packfile_save_packfile: SlotBool<'static>,
    pub packfile_save_packfile_as: SlotBool<'static>,
    pub packfile_change_packfile_type: SlotBool<'static>,
    pub packfile_index_includes_timestamp: SlotBool<'static>,
    pub packfile_data_is_compressed: SlotBool<'static>,
    pub packfile_preferences: SlotBool<'static>,
    pub packfile_quit: SlotBool<'static>,

    //-----------------------------------------------//
    // `View` menu slots.
    //-----------------------------------------------//
    pub view_toggle_packfile_contents: SlotBool<'static>,
    pub view_toggle_global_search_panel: SlotBool<'static>,

    //-----------------------------------------------//
    // `Game Selected` menu slots.
    //-----------------------------------------------//
    pub game_selected_open_game_data_folder: SlotBool<'static>,
    pub game_selected_open_game_assembly_kit_folder: SlotBool<'static>,
    pub change_game_selected: SlotBool<'static>,

    //-----------------------------------------------//
    // `Special Stuff` menu slots.
    //-----------------------------------------------//
    pub special_stuff_generate_pak_file: SlotBool<'static>,
    pub special_stuff_optimize_packfile: SlotBool<'static>,
    pub special_stuff_patch_siege_ai: SlotBool<'static>,

    //-----------------------------------------------//
    // `About` menu slots.
    //-----------------------------------------------//
    pub about_about_qt: SlotBool<'static>,
    pub about_open_manual: SlotBool<'static>,
    pub about_patreon_link: SlotBool<'static>,
}

//-------------------------------------------------------------------------------//
//                             Implementations
//-------------------------------------------------------------------------------//

/// Implementation of `AppUISlots`.
impl AppUISlots {

	/// This function creates an entire `AppUISlots` struct. Used to create the logic of the starting UI.
	pub fn new(
        app_ui: AppUI,
        global_search_ui: GlobalSearchUI,
        pack_file_contents_ui: PackFileContentsUI,
    ) -> Self {

		//-----------------------------------------------//
        // Command Palette logic.
        //-----------------------------------------------//

        // This one puts the command palette in the top center part of the window, make it appear and gives it the focus.
		let command_palette_show = SlotNoArgs::new(move || {
            let line_edit = unsafe { app_ui.command_palette_line_edit.as_mut().unwrap() };
            let command_palette = unsafe { app_ui.command_palette.as_mut().unwrap() };
            let completer = unsafe { app_ui.command_palette_completer.as_mut().unwrap() };
            let main_window = unsafe { app_ui.main_window.as_mut().unwrap() };

            let width = (main_window.width() / 2 ) - (command_palette.width() / 2 );
			let height = 80;
            command_palette.move_((width, height));
            unsafe { line_edit.set_completer(app_ui.command_palette_completer) };

            command_palette::load_actions(&app_ui, &pack_file_contents_ui);
            command_palette.show();
			line_edit.set_focus(FocusReason::Shortcut);
            line_edit.set_text(&QString::from_std_str(""));

            line_edit.completer();
            completer.complete(());
        });

		// This one hides the command palette.
        let command_palette_hide = SlotNoArgs::new(move || {
            unsafe { app_ui.command_palette_line_edit.as_mut().unwrap().set_completer(Completer::new(()).as_mut_ptr()) }
            unsafe { app_ui.command_palette.as_mut().unwrap().hide(); }
        });

        // This is the fun one. This one triggers any command you type in the command palette.
        let command_palette_trigger = SlotStringRef::new(move |command| {
        	unsafe { app_ui.command_palette.as_mut().unwrap().hide(); }
            command_palette::exec_action(&app_ui, &pack_file_contents_ui, command);
        });

        //-----------------------------------------------//
        // `PackFile` menu logic.
        //-----------------------------------------------//

        // What happens when we trigger the "New PackFile" action.
        let packfile_new_packfile = SlotBool::new(move |_| {

                // Check first if there has been changes in the PackFile.
                if app_ui.are_you_sure(false) {

                    // Tell the Background Thread to create a new PackFile.
                    CENTRAL_COMMAND.send_message_qt(Command::NewPackFile);

                    // Disable the main window, so the user can't interrupt the process or iterfere with it.
                    unsafe { (app_ui.main_window.as_mut().unwrap() as &mut Widget).set_enabled(false); }

                    // Close any open PackedFile and clear the global search pannel.
                    // TODO: Clear the global search panel.
                    app_ui.purge_them_all();
                    unsafe { global_search_ui.global_search_dock_widget.as_mut().unwrap().hide(); }
                    //if !SETTINGS.lock().unwrap().settings_bool["remember_table_state_permanently"] { TABLE_STATES_UI.lock().unwrap().clear(); }

                    // Show the "Tips".
                    //display_help_tips(&app_ui);


                    // New PackFiles are always of Mod type.
                    unsafe { app_ui.change_packfile_type_mod.as_mut().unwrap().set_checked(true); }

                    // By default, the four bitmask should be false.
                    unsafe { app_ui.change_packfile_type_data_is_encrypted.as_mut().unwrap().set_checked(false); }
                    unsafe { app_ui.change_packfile_type_index_includes_timestamp.as_mut().unwrap().set_checked(false); }
                    unsafe { app_ui.change_packfile_type_index_is_encrypted.as_mut().unwrap().set_checked(false); }
                    unsafe { app_ui.change_packfile_type_header_is_extended.as_mut().unwrap().set_checked(false); }

                    // We also disable compression by default.
                    unsafe { app_ui.change_packfile_type_data_is_compressed.as_mut().unwrap().set_checked(false); }

                    // Update the TreeView.
                    pack_file_contents_ui.packfile_contents_tree_view.update_treeview(true, TreeViewOperation::Build(false));

                    // Re-enable the Main Window.
                    unsafe { (app_ui.main_window.as_mut().unwrap() as &mut Widget).set_enabled(true); }

                    // Enable the actions available for the PackFile from the `MenuBar`.
                    //enable_packfile_actions(&app_ui, &mymod_stuff, true);

                    // Set the current "Operational Mode" to Normal, as this is a "New" mod.
                    //set_my_mod_mode(&mymod_stuff, &mode, None);

                    // Clean the TableStateData.
                    //*table_state_data.borrow_mut() = TableStateData::new();
                }
            }
        );

        let packfile_open_packfile = SlotBool::new(move |_| {

                // Check first if there has been changes in the PackFile.
                if app_ui.are_you_sure(false) {

                    // Create the FileDialog to get the PackFile to open and configure it.
                    let mut file_dialog = unsafe { FileDialog::new_unsafe((
                        app_ui.main_window as *mut Widget,
                        &QString::from_std_str("Open PackFiles"),
                    )) };
                    file_dialog.set_name_filter(&QString::from_std_str("PackFiles (*.pack)"));
                    file_dialog.set_file_mode(FileMode::ExistingFiles);

                    // Run it and expect a response (1 => Accept, 0 => Cancel).
                    if file_dialog.exec() == 1 {

                        // Now the fun thing. We have to get all the selected files, and then open them one by one.
                        // For that we use the same logic as for the "Load All CA PackFiles" feature.
                        let mut paths = vec![];
                        for index in 0..file_dialog.selected_files().count(()) {
                            paths.push(PathBuf::from(file_dialog.selected_files().at(index).to_std_string()));
                        }

                        // Try to open it, and report it case of error.
                        if let Err(error) = app_ui.open_packfile(&pack_file_contents_ui, &paths, "") { show_dialog(app_ui.main_window as *mut Widget, error, false); }
                    }
                }
            }
        );

        // What happens when we trigger the "Save PackFile" action.
        let packfile_save_packfile = SlotBool::new(move |_| {
                if let Err(error) = app_ui.save_packfile(&pack_file_contents_ui, false) {
                    show_dialog(app_ui.main_window as *mut Widget, error, false);
                }
            }
        );

        // What happens when we trigger the "Save PackFile As" action.
        let packfile_save_packfile_as = SlotBool::new(move |_| {
                if let Err(error) = app_ui.save_packfile(&pack_file_contents_ui, true) {
                    show_dialog(app_ui.main_window as *mut Widget, error, false);
                }
            }
        );

        // What happens when we trigger the "Change PackFile Type" action.
        let packfile_change_packfile_type = SlotBool::new(move |_| {

                // Get the currently selected PackFile's Type.
                let packfile_type = unsafe { match &*(app_ui.change_packfile_type_group.as_ref().unwrap()
                    .checked_action().as_ref().unwrap().text().to_std_string()) {
                    "&Boot" => PFHFileType::Boot,
                    "&Release" => PFHFileType::Release,
                    "&Patch" => PFHFileType::Patch,
                    "&Mod" => PFHFileType::Mod,
                    "Mo&vie" => PFHFileType::Movie,
                    _ => PFHFileType::Other(99),
                } };

                // Send the type to the Background Thread, and update the UI.
                CENTRAL_COMMAND.send_message_qt(Command::SetPackFileType(packfile_type));
                pack_file_contents_ui.packfile_contents_tree_view.update_treeview(true, TreeViewOperation::Modify(vec![TreePathType::PackFile]));
            }
        );

        // What happens when we change the value of "Include Last Modified Date" action.
        let packfile_index_includes_timestamp = SlotBool::new(move |_| {
                let state = unsafe { app_ui.change_packfile_type_index_includes_timestamp.as_ref().unwrap().is_checked() };
                CENTRAL_COMMAND.send_message_qt(Command::ChangeIndexIncludesTimestamp(state));
            }
        );

        // What happens when we enable/disable compression on the current PackFile.
        let packfile_data_is_compressed = SlotBool::new(move |_| {
                let state = unsafe { app_ui.change_packfile_type_data_is_compressed.as_ref().unwrap().is_checked() };
                CENTRAL_COMMAND.send_message_qt(Command::ChangeDataIsCompressed(state));
            }
        );

        // What happens when we trigger the "Preferences" action.
        let packfile_preferences = SlotBool::new(move |_| {

                // We store a copy of the old settings (for checking changes) and trigger the new settings dialog.
                let old_settings = SETTINGS.lock().unwrap().clone();
                if let Some(settings) = SettingsUI::new(&app_ui) {

                    // If we returned new settings, save them and wait for confirmation.
                    CENTRAL_COMMAND.send_message_qt(Command::SetSettings(settings.clone()));
                    match CENTRAL_COMMAND.recv_message_qt() {

                        // If it worked, do some checks to ensure the UI keeps his consistency.
                        Response::Success => {

                            // If we changed the "MyMod's Folder" path, disable the MyMod mode and set it so the MyMod menu will be re-built
                            // next time we open the MyMod menu.
                            if settings.paths["mymods_base_path"] != old_settings.paths["mymods_base_path"] {
                                UI_STATE.set_operational_mode(&app_ui, None);
                                UI_STATE.set_mymod_menu_needs_rebuild(true);
                            }

                            // If we have changed the path of any of the games, and that game is the current `GameSelected`,
                            // re-select the current `GameSelected` to force it to reload the game's files.
                            let has_game_selected_path_changed = settings.paths.iter()
                                .filter(|x| x.0 != "mymods_base_path" && &old_settings.paths[x.0] != x.1)
                                .any(|x| x.0 == &*GAME_SELECTED.lock().unwrap());

                            if has_game_selected_path_changed {
                                unsafe { Action::trigger(app_ui.game_selected_group.as_mut().unwrap().checked_action().as_mut().unwrap()); }
                            }
                        }

                        // If we got an error, report it.
                        Response::Error(error) => show_dialog(app_ui.main_window as *mut Widget, error, false),

                        // In ANY other situation, it's a message problem.
                        _ => panic!(THREADS_COMMUNICATION_ERROR)
                    }
                }
            }
        );

        // What happens when we trigger the "Quit" action.
        let packfile_quit = SlotBool::new(clone!(
            app_ui => move |_| {
                if app_ui.are_you_sure(false) {
                    unsafe { app_ui.main_window.as_mut().unwrap().close(); }
                }
            }
        ));

        //-----------------------------------------------//
        // `View` menu logic.
        //-----------------------------------------------//
        let view_toggle_packfile_contents = SlotBool::new(move |_| {
            let is_visible = unsafe { pack_file_contents_ui.packfile_contents_dock_widget.as_mut().unwrap().is_visible() };
            if is_visible { unsafe { pack_file_contents_ui.packfile_contents_dock_widget.as_mut().unwrap().hide(); }}
            else {unsafe { pack_file_contents_ui.packfile_contents_dock_widget.as_mut().unwrap().show(); }}
        });

        let view_toggle_global_search_panel = SlotBool::new(move |_| {
            let is_visible = unsafe { global_search_ui.global_search_dock_widget.as_mut().unwrap().is_visible() };
            if is_visible { unsafe { global_search_ui.global_search_dock_widget.as_mut().unwrap().hide(); }}
            else {unsafe { global_search_ui.global_search_dock_widget.as_mut().unwrap().show(); }}
        });

        //-----------------------------------------------//
        // `Game Selected` menu logic.
        //-----------------------------------------------//

        // What happens when we trigger the "Open Game's Data Folder" action.
        let game_selected_open_game_data_folder = SlotBool::new(move |_| {
            if let Some(path) = get_game_selected_data_path(&*GAME_SELECTED.lock().unwrap()) {
                if open::that(&path).is_err() {
                    show_dialog(app_ui.main_window as *mut Widget, ErrorKind::IOFolderCannotBeOpened, false);
                };
            }
            else { show_dialog(app_ui.main_window as *mut Widget, ErrorKind::GamePathNotConfigured, false); }
        });

        // What happens when we trigger the "Open Game's Assembly Kit Folder" action.
        let game_selected_open_game_assembly_kit_folder = SlotBool::new(move |_| {
            if let Some(path) = get_game_selected_assembly_kit_path(&*GAME_SELECTED.lock().unwrap()) {
                if open::that(&path).is_err() {
                    show_dialog(app_ui.main_window as *mut Widget, ErrorKind::IOFolderCannotBeOpened, false);
                };
            }
            else { show_dialog(app_ui.main_window as *mut Widget, ErrorKind::GamePathNotConfigured, false); }
        });

        // What happens when we trigger the "Change Game Selected" action.
        let change_game_selected = SlotBool::new(move |_| {

                // Get the new `Game Selected` and clean his name up, so it ends up like "x_y".
                let mut new_game_selected = unsafe { app_ui.game_selected_group.as_mut().unwrap().checked_action().as_mut().unwrap().text().to_std_string() };
                if let Some(index) = new_game_selected.find('&') { new_game_selected.remove(index); }
                let new_game_selected = new_game_selected.replace(' ', "_").to_lowercase();

                // Disable the Main Window (so we can't do other stuff).
                unsafe { (app_ui.main_window.as_mut().unwrap() as &mut Widget).set_enabled(false); }

                // Send the command to the background thread to set the new `Game Selected`, and tell RPFM to rebuild the mymod menu when it can.
                CENTRAL_COMMAND.send_message_qt(Command::SetGameSelected(new_game_selected));
                UI_STATE.set_mymod_menu_needs_rebuild(true);

                // Disable the `PackFile Management` actions and, if we have a `PackFile` open, re-enable them.
                app_ui.enable_packfile_actions(false);
                if unsafe { pack_file_contents_ui.packfile_contents_tree_model.as_ref().unwrap().row_count(()) } != 0 {
                    app_ui.enable_packfile_actions(true);
                }

                // Set the current "Operational Mode" to `Normal` (In case we were in `MyMod` mode).
                UI_STATE.set_operational_mode(&app_ui, None);

                // Re-enable the Main Window.
                unsafe { (app_ui.main_window.as_mut().unwrap() as &mut Widget).set_enabled(true); }

                // Change the GameSelected Icon. Disabled until we find better icons.
                GameSelectedIcons::set_game_selected_icon(app_ui.main_window);
            }
        );

        //-----------------------------------------------------//
        // `Special Stuff` menu logic.
        //-----------------------------------------------------//

        // What happens when we trigger the "Generate Pak File" action.
        let special_stuff_generate_pak_file = SlotBool::new(move |_| {

                // For Rome 2+, we need the game path set. For other games, we have to ask for a path.
                let version = SUPPORTED_GAMES.get(&**GAME_SELECTED.lock().unwrap()).unwrap().raw_db_version;
                let path = match version {

                    // Post-Shogun 2 games.
                    2 => {
                        let mut path = SETTINGS.lock().unwrap().paths[&**GAME_SELECTED.lock().unwrap()].clone().unwrap();
                        path.push("assembly_kit");
                        path.push("raw_data");
                        path.push("db");
                        path
                    }

                    // Shogun 2.
                    1 => {

                        // Create the FileDialog to get the path of the Assembly Kit.
                        let mut file_dialog = unsafe { FileDialog::new_unsafe((
                            app_ui.main_window as *mut Widget,
                            &QString::from_std_str("Select Assembly Kit's Folder"),
                        )) };

                        // Set it to only search Folders.
                        file_dialog.set_file_mode(FileMode::Directory);
                        file_dialog.set_option(ShowDirsOnly);

                        // Run it and expect a response (1 => Accept, 0 => Cancel).
                        let mut path = if file_dialog.exec() == 1 { PathBuf::from(file_dialog.selected_files().at(0).to_std_string())
                        } else { PathBuf::from("") };
                        path.push("raw_data");
                        path.push("db");
                        path
                    }

                    // Empire and Napoleon. This is not really supported yet. It's leave here as a placeholder.
                    0 => {

                        // Create the FileDialog to get the path of the Assembly Kit.
                        let mut file_dialog = unsafe { FileDialog::new_unsafe((
                            app_ui.main_window as *mut Widget,
                            &QString::from_std_str("Select Raw DB Folder"),
                        )) };

                        // Set it to only search Folders.
                        file_dialog.set_file_mode(FileMode::Directory);
                        file_dialog.set_option(ShowDirsOnly);

                        // Run it and expect a response (1 => Accept, 0 => Cancel).
                        if file_dialog.exec() == 1 { PathBuf::from(file_dialog.selected_files().at(0).to_std_string())
                        } else { PathBuf::from("") }
                    }

                    // For any other game, return an empty path.
                    _ => PathBuf::new(),
                };

                if path.file_name().is_some() {

                    // If there is no problem, ere we go.
                    unsafe { (app_ui.main_window.as_mut().unwrap() as &mut Widget).set_enabled(false); }

                    CENTRAL_COMMAND.send_message_qt(Command::GeneratePakFile(path, version));
                    match CENTRAL_COMMAND.recv_message_qt_try() {
                        Response::Success => show_dialog(app_ui.main_window as *mut Widget, "PAK File succesfully created and reloaded.", true),
                        Response::Error(error) => show_dialog(app_ui.main_window as *mut Widget, error, false),
                        _ => panic!(THREADS_COMMUNICATION_ERROR),
                    }

                    unsafe { (app_ui.main_window.as_mut().unwrap() as &mut Widget).set_enabled(true); }
                }
                else {
                    show_dialog(app_ui.main_window as *mut Widget, "This operation is not supported for the Game Selected.", false);
                }
            }
        );

        // What happens when we trigger the "Optimize PackFile" action.
        let special_stuff_optimize_packfile = SlotBool::new(move |_| {

                // This cannot be done if there is a PackedFile open. Well, can be done, but it's a pain in the ass to do it.
                if !UI_STATE.get_open_packedfiles().is_empty() {
                    return show_dialog(app_ui.main_window as *mut Widget, ErrorKind::OperationNotAllowedWithPackedFileOpen, false);
                }

                // If there is no problem, ere we go.
                unsafe { (app_ui.main_window.as_mut().unwrap() as &mut Widget).set_enabled(false); }

                CENTRAL_COMMAND.send_message_qt(Command::OptimizePackFile);
                match CENTRAL_COMMAND.recv_message_qt_try() {
                    Response::VecVecString(response) => {
                        let response = response.iter().map(|x| TreePathType::File(x.to_vec())).collect::<Vec<TreePathType>>();

                        pack_file_contents_ui.packfile_contents_tree_view.update_treeview(true, TreeViewOperation::Delete(response));
                        show_dialog(app_ui.main_window as *mut Widget, "PackFile optimized.", true);

                        // Update the global search stuff, if needed.
                        //unsafe { update_global_search_stuff.as_mut().unwrap().trigger(); }
                    }
                    _ => panic!(THREADS_COMMUNICATION_ERROR),
                }

                // Re-enable the Main Window.
                unsafe { (app_ui.main_window.as_mut().unwrap() as &mut Widget).set_enabled(true); }
            }
        );

        // What happens when we trigger the "Patch Siege AI" action.
        let special_stuff_patch_siege_ai = SlotBool::new(move |_| {

                // Ask the background loop to patch the PackFile, and wait for a response.
                unsafe { (app_ui.main_window.as_mut().unwrap() as &mut Widget).set_enabled(false); }
                CENTRAL_COMMAND.send_message_qt(Command::PatchSiegeAI);
                match CENTRAL_COMMAND.recv_message_qt_try() {
                    Response::StringVecVecString(response) => {
                        let message = response.0;
                        let paths = response.1.iter().map(|x| TreePathType::File(x.to_vec())).collect::<Vec<TreePathType>>();;
                        pack_file_contents_ui.packfile_contents_tree_view.update_treeview(true, TreeViewOperation::Delete(paths));
                        show_dialog(app_ui.main_window as *mut Widget, &message, true);
                    }

                    // If the PackFile is empty or is not patchable, report it. Otherwise, praise the nine divines.
                    Response::Error(error) => show_dialog(app_ui.main_window as *mut Widget, error, false),
                    _ => panic!(THREADS_COMMUNICATION_ERROR)
                }

                // Re-enable the Main Window.
                unsafe { (app_ui.main_window.as_mut().unwrap() as &mut Widget).set_enabled(true); }
            }
        );

		//-----------------------------------------------//
        // `About` menu logic.
        //-----------------------------------------------//

        // What happens when we trigger the "About Qt" action.
        let about_about_qt = SlotBool::new(move |_| { unsafe { MessageBox::about_qt(app_ui.main_window as *mut Widget); }});

        // What happens when we trigger the "Open Manual" action.
        let about_open_manual = SlotBool::new(|_| { DesktopServices::open_url(&qt_core::url::Url::new(&QString::from_std_str(DOCS_BASE_URL))); });

        // What happens when we trigger the "Support me on Patreon" action.
        let about_patreon_link = SlotBool::new(|_| { DesktopServices::open_url(&qt_core::url::Url::new(&QString::from_std_str(PATREON_URL))); });

        // And here... we return all the slots.
		Self {

			//-----------------------------------------------//
	        // Command Palette slots.
	        //-----------------------------------------------//
			command_palette_show,
    		command_palette_hide,
    		command_palette_trigger,

            //-----------------------------------------------//
            // `PackFile` menu slots.
            //-----------------------------------------------//
            packfile_new_packfile,
            packfile_open_packfile,
            packfile_save_packfile,
            packfile_save_packfile_as,
            packfile_change_packfile_type,
            packfile_index_includes_timestamp,
            packfile_data_is_compressed,
            packfile_preferences,
            packfile_quit,

            //-----------------------------------------------//
            // `View` menu slots.
            //-----------------------------------------------//
            view_toggle_packfile_contents,
            view_toggle_global_search_panel,

            //-----------------------------------------------//
            // `Game Selected` menu slots.
            //-----------------------------------------------//
            game_selected_open_game_data_folder,
            game_selected_open_game_assembly_kit_folder,
            change_game_selected,

            //-----------------------------------------------//
            // `Special Stuff` menu slots.
            //-----------------------------------------------//
            special_stuff_generate_pak_file,
            special_stuff_optimize_packfile,
            special_stuff_patch_siege_ai,

    		//-----------------------------------------------//
	        // `About` menu slots.
	        //-----------------------------------------------//
    		about_about_qt,
            about_open_manual,
            about_patreon_link,
		}
	}
}
