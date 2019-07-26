//---------------------------------------------------------------------------//
// Copyright (c) 2017-2019 Ismael Gutiérrez González. All rights reserved.
// 
// This file is part of the Rusted PackFile Manager (RPFM) project,
// which can be found here: https://github.com/Frodo45127/rpfm.
// 
// This file is licensed under the MIT license, which can be found here:
// https://github.com/Frodo45127/rpfm/blob/master/LICENSE.
//---------------------------------------------------------------------------//

use rpfm_error::Result;
use rpfm_lib::settings::Settings;
use rpfm_lib::schema::Schema;
use rpfm_lib::SUPPORTED_GAMES;

/// Struct `Config`: This struct serves to hold the configuration used during the execution of the program:
/// 
pub struct Config {
	pub game_selected: String,
	pub schema: Schema,
	pub settings: Settings,
	pub verbosity_level: u64,
}

/// Implementation of `Config`.
impl Config {

	/// This function creates a new Config struct configured for the provided game.
	pub fn new(game_selected: String, settings: Settings, verbosity_level: u64) -> Result<Self> {
		Ok(Self {
			schema: Schema::load(&SUPPORTED_GAMES[&*game_selected].schema)?,
			game_selected,
			settings,
			verbosity_level,
		})
	}
}