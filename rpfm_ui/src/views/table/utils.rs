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
In this file are all the utility functions we need for the tables to work.
!*/

use qt_widgets::QDialog;
use qt_widgets::QTableView;
use qt_widgets::q_header_view::ResizeMode;

use qt_gui::QListOfQStandardItem;
use qt_gui::QStandardItem;
use qt_gui::QStandardItemModel;

use qt_core::QModelIndex;
use qt_core::QSignalBlocker;
use qt_core::QSortFilterProxyModel;
use qt_core::QVariant;
use qt_core::QObject;
use qt_core::CheckState;
use qt_core::QString;
use qt_core::Orientation;
use qt_core::SortOrder;

use cpp_core::CppBox;
use cpp_core::Ptr;
use cpp_core::Ref;

use std::collections::BTreeMap;
use std::cmp::{Ordering, Reverse};
use std::rc::Rc;
use std::sync::{atomic::AtomicPtr, RwLock};

use rpfm_lib::packedfile::table::{DependencyData, Table};
use rpfm_lib::schema::{Definition, Field, FieldType};
use rpfm_lib::SETTINGS;

use crate::ffi::*;
use crate::locale::{qtr, tr, tre};
use crate::utils::*;
use crate::UI_STATE;
use super::*;

//----------------------------------------------------------------------------//
//                       Undo/Redo helpers for tables
//----------------------------------------------------------------------------//

/// This function is used to update the background or undo table when a change is made in the main table.
pub unsafe fn update_undo_model(model: &QPtr<QStandardItemModel>, undo_model: &QPtr<QStandardItemModel>) {
    undo_model.clear();
    for row in 0..model.row_count_0a() {
        for column in 0..model.column_count_0a() {
            let item = &*model.item_2a(row, column);
            undo_model.set_item_3a(row, column, item.clone());
        }
    }
}

//----------------------------------------------------------------------------//
//                       Index helpers for tables
//----------------------------------------------------------------------------//

/// This function sorts the VISUAL SELECTION. That means, the selection just as you see it on screen.
/// This should be provided with the indexes OF THE VIEW/FILTER, NOT THE MODEL.
pub unsafe fn sort_indexes_visually(indexes_sorted: &mut Vec<Ref<QModelIndex>>, table_view: &QPtr<QTableView>) {

    // Sort the indexes so they follow the visual index, not their logical one.
    // This should fix situations like copying a row and getting a different order in the cells,
    // or copying a sorted table and getting a weird order in the copied cells.
    let horizontal_header = table_view.horizontal_header();
    let vertical_header = table_view.vertical_header();
    indexes_sorted.sort_unstable_by(|a, b| {
        let cmp = vertical_header.visual_index(a.row()).cmp(&vertical_header.visual_index(b.row()));
        match cmp {
            Ordering::Equal => if horizontal_header.visual_index(a.column()) < horizontal_header.visual_index(b.column()) { Ordering::Less } else { Ordering::Greater },
            _ => cmp,
        }
    });
}

/// This function sorts the MODEL SELECTION. That means, the real selection over the model.
/// This should be provided with the indexes OF THE MODEL, NOT THE VIEW/FILTER.
pub unsafe fn sort_indexes_by_model(indexes_sorted: &mut Vec<Ref<QModelIndex>>) {

    // Sort the indexes so they follow the visual index, not their logical one.
    // This should fix situations like copying a row and getting a different order in the cells,
    // or copying a sorted table and getting a weird order in the copied cells.
    indexes_sorted.sort_unstable_by(|a, b| {
        let cmp = a.row().cmp(&b.row());
        match cmp {
            Ordering::Equal => if a.column() < b.column() { Ordering::Less } else { Ordering::Greater },
            _ => cmp,
        }
    });
}


/// This function gives you the model's ModelIndexes from the ones from the view/filter.
pub unsafe fn get_real_indexes(indexes_sorted: &[Ref<QModelIndex>], filter_model: &QPtr<QSortFilterProxyModel>) -> Vec<CppBox<QModelIndex>> {
    indexes_sorted.iter().map(|x| filter_model.map_to_source(*x)).collect()
}

/// This function removes indexes with the same row from a list of indexes.
pub unsafe fn dedup_indexes_per_row(indexes: &mut Vec<Ref<QModelIndex>>) {
    let mut rows_done = vec![];
    let mut indexes_to_remove = vec![];
    for (pos, index) in indexes.iter().enumerate() {
        if rows_done.contains(&index.row()) { indexes_to_remove.push(pos); }
        else { rows_done.push(index.row())}
    }

    for index_to_remove in indexes_to_remove.iter().rev() {
        indexes.remove(*index_to_remove);
    }
}

/// This function deletes the provided rows from the provided model.
///
/// It returns a list of (first row of the pack's position, list of deleted rows).
/// NOTE: The list of rows must be in 9->0 order.
pub unsafe fn delete_rows(model: &QPtr<QStandardItemModel>, rows: &[i32]) -> Vec<(i32, Vec<Vec<AtomicPtr<QStandardItem>>>)> {

    // Make sure all rows are sorted 9->0.
    let mut rows = rows.to_vec();
    rows.sort_by_key(|&y| Reverse(y));

    // To optimize this, we remove them in consecutive packs, which really helps when you're deleting a ton of rows at the same time.
    // That way we only trigger one deletion with it's signals instead a ton of them.
    let mut rows_splitted = vec![];
    let mut current_row_pack = vec![];
    let mut current_row_index = -2;
    for (index, row) in rows.iter().enumerate() {

        // Items are individually cloned because there is no "takeRows" function to take out multiple individual rows.
        let items = (0..model.column_count_0a())
            .map(|column| (&*model.item_2a(*row, column)).clone())
            .collect::<Vec<Ptr<QStandardItem>>>();

        // If the current line is not the next of the batch, nor the first one, finish the pack.
        if (*row != current_row_index - 1) && index != 0 {
            current_row_pack.reverse();
            rows_splitted.push((current_row_index, current_row_pack.to_vec()));
            current_row_pack.clear();
        }

        // Add the new row to the current pack.
        current_row_pack.push(items);
        current_row_index = *row;
    }
    current_row_pack.reverse();
    rows_splitted.push((current_row_index, current_row_pack));

    // And finally, remove the rows from the table.
    for row_pack in rows_splitted.iter() {
        model.remove_rows_2a(row_pack.0, row_pack.1.len() as i32);
    }

    // Reverse them, so the final result is full top to bottom, and return them.
    rows_splitted.reverse();
    rows_splitted.iter()
        .map(|x| (x.0, x.1.iter()
            .map(|y| y.iter()
                .map(|z| atomic_from_ptr(*z))
                .collect()
            )
            .collect()
        ))
        .collect::<Vec<(i32, Vec<Vec<AtomicPtr<QStandardItem>>>)>>()
}

/// This function returns a new default row.
pub unsafe fn get_new_row(table_definition: &Definition) -> CppBox<QListOfQStandardItem> {
    let qlist = QListOfQStandardItem::new();
    for field in table_definition.get_fields_processed() {
        let item = get_default_item_from_field(&field);
        qlist.append_q_standard_item(&item.into_ptr().as_mut_raw_ptr());
    }
    qlist
}

/// This function generates a *Default* StandardItem for the provided field.
pub unsafe fn get_default_item_from_field(field: &Field) -> CppBox<QStandardItem> {
    match field.get_ref_field_type() {
        FieldType::Boolean => {
            let item = QStandardItem::new();
            item.set_editable(false);
            item.set_checkable(true);
            item.set_data_2a(&QVariant::from_bool(true), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(false), ITEM_IS_SEQUENCE);

            let check_state = if let Some(default_value) = field.get_default_value() {
                default_value.to_lowercase() == "true"
            } else { false };

            if check_state {
                item.set_check_state(CheckState::Checked);
                item.set_data_2a(&QVariant::from_bool(true), ITEM_SOURCE_VALUE);
                item.set_tool_tip(&QString::from_std_str(&tre("original_data", &["True"])));
            }
            else {
                item.set_check_state(CheckState::Unchecked);
                item.set_data_2a(&QVariant::from_bool(false), ITEM_SOURCE_VALUE);
                item.set_tool_tip(&QString::from_std_str(&tre("original_data", &["False"])));
            }
            item
        }
        FieldType::F32 => {
            let item = QStandardItem::new();
            let data = if let Some(default_value) = field.get_default_value() {
                if let Ok(default_value) = default_value.parse::<f32>() {
                    default_value
                } else {
                    0.0f32
                }
            } else {
                0.0f32
            };

            item.set_tool_tip(&QString::from_std_str(&tre("original_data", &[&data.to_string()])));
            item.set_data_2a(&QVariant::from_bool(true), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(false), ITEM_IS_SEQUENCE);
            item.set_data_2a(&QVariant::from_float(data), ITEM_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_float(data), 2);
            item
        },
        FieldType::I16 => {
            let item = QStandardItem::new();
            let data = if let Some(default_value) = field.get_default_value() {
                if let Ok(default_value) = default_value.parse::<i16>() {
                    default_value as i32
                } else {
                    0_i32
                }
            } else {
                0_i32
            };
            item.set_tool_tip(&QString::from_std_str(&tre("original_data", &[&data.to_string()])));
            item.set_data_2a(&QVariant::from_bool(true), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(false), ITEM_IS_SEQUENCE);
            item.set_data_2a(&QVariant::from_int(data), ITEM_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_int(data), 2);
            item
        },
        FieldType::I32 => {
            let item = QStandardItem::new();
            let data = if let Some(default_value) = field.get_default_value() {
                if let Ok(default_value) = default_value.parse::<i32>() {
                    default_value
                } else {
                    0i32
                }
            } else {
                0i32
            };
            item.set_tool_tip(&QString::from_std_str(&tre("original_data", &[&data.to_string()])));
            item.set_data_2a(&QVariant::from_bool(true), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(false), ITEM_IS_SEQUENCE);
            item.set_data_2a(&QVariant::from_int(data), ITEM_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_int(data), 2);
            item
        },
        FieldType::I64 => {
            let item = QStandardItem::new();
            let data = if let Some(default_value) = field.get_default_value() {
                if let Ok(default_value) = default_value.parse::<i64>() {
                    default_value
                } else {
                    0i64
                }
            } else {
                0i64
            };
            item.set_tool_tip(&QString::from_std_str(&tre("original_data", &[&data.to_string()])));
            item.set_data_2a(&QVariant::from_bool(true), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(false), ITEM_IS_SEQUENCE);
            item.set_data_2a(&QVariant::from_i64(data), ITEM_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_i64(data), 2);
            item
        },
        FieldType::StringU8 |
        FieldType::StringU16 |
        FieldType::OptionalStringU8 |
        FieldType::OptionalStringU16 => {
            let text = if let Some(default_value) = field.get_default_value() {
                default_value.to_owned()
            } else {
                String::new()
            };
            let item = QStandardItem::from_q_string(&QString::from_std_str(&text));
            item.set_tool_tip(&QString::from_std_str(&tre("original_data", &[&text])));
            item.set_data_2a(&QVariant::from_bool(true), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(false), ITEM_IS_SEQUENCE);
            item.set_data_2a(&QVariant::from_q_string(&QString::from_std_str(&text)), ITEM_SOURCE_VALUE);
            item
        },

        FieldType::SequenceU16(ref definition) | FieldType::SequenceU32(ref definition)  => {
            let table = serde_json::to_string(&Table::new(&definition)).unwrap();
            let item = QStandardItem::new();

            item.set_text(&qtr("packedfile_editable_sequence"));
            item.set_data_2a(&QVariant::from_bool(false), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(true), ITEM_IS_SEQUENCE);
            item.set_data_2a(&QVariant::from_q_string(&QString::from_std_str(&table)), ITEM_SEQUENCE_DATA);
            item
        }
    }
}

/// This function "process" the column names of a table, so they look like they should.
pub fn clean_column_names(field_name: &str) -> String {
    let mut new_name = String::new();
    let mut should_be_uppercase = false;

    for character in field_name.chars() {

        if new_name.is_empty() || should_be_uppercase {
            new_name.push_str(&character.to_uppercase().to_string());
            should_be_uppercase = false;
        }

        else if character == '_' {
            new_name.push(' ');
            should_be_uppercase = true;
        }

        else { new_name.push(character); }
    }

    new_name
}

/// This function loads the data from a compatible `PackedFile` into a TableView.
pub unsafe fn load_data(
    table_view_primary: &QPtr<QTableView>,
    table_view_frozen: &QPtr<QTableView>,
    definition: &Definition,
    dependency_data: &RwLock<BTreeMap<i32, DependencyData>>,
    data: &TableType,
) {
    let table_filter: QPtr<QSortFilterProxyModel> = table_view_primary.model().static_downcast();
    let table_model: QPtr<QStandardItemModel> = table_filter.source_model().static_downcast();

    // First, we delete all the data from the `ListStore`. Just in case there is something there.
    // This wipes out header information, so remember to run "build_columns" after this.
    table_model.clear();

    // Set the right data, depending on the table type you get.
    let data = match data {
        TableType::AnimFragment(data) => data.get_ref_table_data(),
        TableType::AnimTable(data) => data.get_ref_table_data(),
        TableType::DependencyManager(data) => &**data,
        TableType::DB(data) => data.get_ref_table_data(),
        TableType::Loc(data) => data.get_ref_table_data(),
        TableType::MatchedCombat(data) => data.get_ref_table_data(),
        TableType::NormalTable(data) => data.get_ref_table_data(),
    };

    if !data.is_empty() {

        // Load the data, row by row.
        let blocker = QSignalBlocker::from_q_object(table_model.static_upcast::<QObject>());
        for (row, entry) in data.iter().enumerate() {
            let qlist = QListOfQStandardItem::new();
            for field in entry {
                let item = get_item_from_decoded_data(field);
                qlist.append_q_standard_item(&item.into_ptr().as_mut_raw_ptr());
            }
            if row == data.len() - 1 {
                blocker.unblock();
            }
            table_model.append_row_q_list_of_q_standard_item(&qlist);
        }
    }

    // If the table it's empty, we add an empty row and delete it, so the "columns" get created.
    else {
        let qlist = get_new_row(&definition);
        table_model.append_row_q_list_of_q_standard_item(&qlist);
        table_model.remove_rows_2a(0, 1);
    }

    setup_item_delegates(
        table_view_primary,
        &table_view_frozen,
        definition,
        &dependency_data.read().unwrap(),
    )
}

/// This function generates a StandardItem for the provided DecodedData.
pub unsafe fn get_item_from_decoded_data(data: &DecodedData) -> CppBox<QStandardItem> {
    match *data {

        // This one needs a couple of changes before turning it into an item in the table.
        DecodedData::Boolean(ref data) => {
            let item = QStandardItem::new();
            item.set_data_2a(&QVariant::from_bool(true), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(false), ITEM_IS_SEQUENCE);
            item.set_data_2a(&QVariant::from_bool(*data), ITEM_SOURCE_VALUE);
            item.set_tool_tip(&QString::from_std_str(&tre("original_data", &[&data.to_string()])));
            item.set_editable(false);
            item.set_checkable(true);
            item.set_check_state(if *data { CheckState::Checked } else { CheckState::Unchecked });
            item
        }

        // Floats need to be tweaked to fix trailing zeroes and precission issues, like turning 0.5000004 into 0.5.
        // Also, they should be limited to 3 decimals.
        DecodedData::F32(ref data) => {
            let data = {
                let data_str = format!("{}", data);
                if let Some(position) = data_str.find('.') {
                    let decimals = &data_str[position..].len();
                    if *decimals > 3 { format!("{:.3}", data).parse::<f32>().unwrap() }
                    else { *data }
                }
                else { *data }
            };

            let item = QStandardItem::new();
            item.set_tool_tip(&QString::from_std_str(&tre("original_data", &[&data.to_string()])));
            item.set_data_2a(&QVariant::from_bool(true), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(false), ITEM_IS_SEQUENCE);
            item.set_data_2a(&QVariant::from_float(data), ITEM_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_float(data), 2);
            item
        },
        DecodedData::I16(ref data) => {
            let item = QStandardItem::new();
            item.set_tool_tip(&QString::from_std_str(tre("original_data", &[&data.to_string()])));
            item.set_data_2a(&QVariant::from_bool(true), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(false), ITEM_IS_SEQUENCE);
            item.set_data_2a(&QVariant::from_int(*data as i32), ITEM_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_int(*data as i32), 2);
            item
        },
        DecodedData::I32(ref data) => {
            let item = QStandardItem::new();
            item.set_tool_tip(&QString::from_std_str(tre("original_data", &[&data.to_string()])));
            item.set_data_2a(&QVariant::from_bool(true), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(false), ITEM_IS_SEQUENCE);
            item.set_data_2a(&QVariant::from_int(*data), ITEM_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_int(*data), 2);
            item
        },
        DecodedData::I64(ref data) => {
            let item = QStandardItem::new();
            item.set_tool_tip(&QString::from_std_str(&tre("original_data", &[&data.to_string()])));
            item.set_data_2a(&QVariant::from_bool(true), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(false), ITEM_IS_SEQUENCE);
            item.set_data_2a(&QVariant::from_i64(*data), ITEM_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_i64(*data), 2);
            item
        },
        // All these are Strings, so it can be together,
        DecodedData::StringU8(ref data) |
        DecodedData::StringU16(ref data) |
        DecodedData::OptionalStringU8(ref data) |
        DecodedData::OptionalStringU16(ref data) => {
            let item = QStandardItem::from_q_string(&QString::from_std_str(data));
            item.set_tool_tip(&QString::from_std_str(&tre("original_data", &[&data])));
            item.set_data_2a(&QVariant::from_bool(true), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(false), ITEM_IS_SEQUENCE);
            item.set_data_2a(&QVariant::from_q_string(&QString::from_std_str(data)), ITEM_SOURCE_VALUE);
            item
        },
        DecodedData::SequenceU16(ref table) | DecodedData::SequenceU32(ref table) => {
            let table = QString::from_std_str(&serde_json::to_string(&table).unwrap());
            let item = QStandardItem::from_q_string(&qtr("packedfile_editable_sequence"));
            item.set_editable(false);
            item.set_data_2a(&QVariant::from_bool(false), ITEM_HAS_SOURCE_VALUE);
            item.set_data_2a(&QVariant::from_bool(true), ITEM_IS_SEQUENCE);
            item.set_data_2a(&QVariant::from_q_string(&table), ITEM_SEQUENCE_DATA);
            item
        }
    }
}

/// This function is meant to be used to prepare and build the column headers, and the column-related stuff.
/// His intended use is for just after we load/reload the data to the table.
pub unsafe fn build_columns(
    table_view_primary: &QPtr<QTableView>,
    table_view_frozen: Option<&QPtr<QTableView>>,
    definition: &Definition,
    table_name: Option<&String>,
) {
    let filter: QPtr<QSortFilterProxyModel> = table_view_primary.model().static_downcast();
    let model: QPtr<QStandardItemModel> = filter.source_model().static_downcast();
    let schema = SCHEMA.read().unwrap();
    let mut do_we_have_ca_order = false;
    let mut keys = vec![];

    for (index, field) in definition.get_fields_processed().iter().enumerate() {

        let name = clean_column_names(&field.get_name());
        let item = QStandardItem::from_q_string(&QString::from_std_str(&name));
        set_column_tooltip(&schema, &field, table_name, &item);
        model.set_horizontal_header_item(index as i32, item.into_ptr());

        // Depending on his type, set one width or another.
        match field.get_ref_field_type() {
            FieldType::Boolean => table_view_primary.set_column_width(index as i32, COLUMN_SIZE_BOOLEAN),
            FieldType::F32 => table_view_primary.set_column_width(index as i32, COLUMN_SIZE_NUMBER),
            FieldType::I16 => table_view_primary.set_column_width(index as i32, COLUMN_SIZE_NUMBER),
            FieldType::I32 => table_view_primary.set_column_width(index as i32, COLUMN_SIZE_NUMBER),
            FieldType::I64 => table_view_primary.set_column_width(index as i32, COLUMN_SIZE_NUMBER),
            FieldType::StringU8 => table_view_primary.set_column_width(index as i32, COLUMN_SIZE_STRING),
            FieldType::StringU16 => table_view_primary.set_column_width(index as i32, COLUMN_SIZE_STRING),
            FieldType::OptionalStringU8 => table_view_primary.set_column_width(index as i32, COLUMN_SIZE_STRING),
            FieldType::OptionalStringU16 => table_view_primary.set_column_width(index as i32, COLUMN_SIZE_STRING),
            FieldType::SequenceU16(_) | FieldType::SequenceU32(_) => table_view_primary.set_column_width(index as i32, COLUMN_SIZE_STRING),
        }

        // If the field is key, add that column to the "Key" list, so we can move them at the beginning later.
        if field.get_is_key() { keys.push(index); }
        if field.get_ca_order() != -1 { do_we_have_ca_order |= true; }
    }

    // Now the order. If we have a sort order from the schema, we use that one.
    if !SETTINGS.read().unwrap().settings_bool["tables_use_old_column_order"] && do_we_have_ca_order {
        let header_primary = table_view_primary.horizontal_header();
        let mut fields = definition.get_fields_processed().iter()
            .enumerate()
            .map(|(x, y)| (x, y.get_ca_order()))
            .collect::<Vec<(usize, i16)>>();
        fields.sort_by(|a, b| {
            if a.1 == -1 || b.1 == -1 { Ordering::Equal }
            else { a.1.cmp(&b.1) }
        });

        for (new_pos, (logical_index, ca_order)) in fields.iter().enumerate() {
            if *ca_order != -1 {
                let visual_index = header_primary.visual_index(*logical_index as i32);
                header_primary.move_section(visual_index as i32, new_pos as i32);

                if let Some(ref table_view_frozen) = table_view_frozen {
                    let header_frozen = table_view_frozen.horizontal_header();
                    header_frozen.move_section(visual_index as i32, new_pos as i32);
                }
            }
        }
    }

    // Otherwise, if we have any "Key" field, move it to the beginning.
    else if !keys.is_empty() {
        let header_primary = table_view_primary.horizontal_header();
        for (position, column) in keys.iter().enumerate() {
            header_primary.move_section(*column as i32, position as i32);

            if let Some(ref table_view_frozen) = table_view_frozen {
                let header_frozen = table_view_frozen.horizontal_header();
                header_frozen.move_section(*column as i32, position as i32);
            }
        }
    }

    // If we want to let the columns resize themselfs...
    if SETTINGS.read().unwrap().settings_bool["adjust_columns_to_content"] {
        table_view_primary.horizontal_header().resize_sections(ResizeMode::ResizeToContents);
    }
}

/// This function sets the tooltip for the provided column header, if the column should have one.
pub unsafe fn set_column_tooltip(
    schema: &Option<Schema>,
    field: &Field,
    table_name: Option<&String>,
    item: &QStandardItem
) {

    // If we passed it a table name, build the tooltip based on it. The logic is simple:
    // - If we have a description, we add it to the tooltip.
    // - If the column references another column, we add it to the tooltip.
    // - If the column is referenced by another column, we add it to the tooltip.
    if let Some(table_name) = table_name {
        let mut tooltip_text = String::new();
        if !field.get_description().is_empty() {
            tooltip_text.push_str(&format!("<p>{}</p>", field.get_description()));
        }

        if let Some(ref reference) = field.get_is_reference() {
            tooltip_text.push_str(&format!("<p>{}</p><p><i>\"{}/{}\"</i></p>", tr("column_tooltip_1"), reference.0, reference.1));
        }

        else {
            let mut referenced_columns = if let Some(ref schema) = schema {
                let short_table_name = if table_name.ends_with("_tables") { table_name.split_at(table_name.len() - 7).0 } else { table_name };
                let mut columns = vec![];

                // We get all the db definitions from the schema, then iterate all of them to find what tables reference our own.
                for versioned_file in schema.get_ref_versioned_file_db_all() {
                    if let VersionedFile::DB(ref_table_name, ref_definition) = versioned_file {
                        let mut found = false;
                        for ref_version in ref_definition {
                            for ref_field in ref_version.get_fields_processed() {
                                if let Some((ref_ref_table, ref_ref_field)) = ref_field.get_is_reference() {
                                    if ref_ref_table == short_table_name && ref_ref_field == field.get_name() {
                                        found = true;
                                        columns.push((ref_table_name.to_owned(), ref_field.get_name().to_owned()));
                                    }
                                }
                            }
                            if found { break; }
                        }
                    }
                }
                columns
            } else { vec![] };

            referenced_columns.sort_unstable();
            if !referenced_columns.is_empty() {
                tooltip_text.push_str(&format!("<p>{}</p>", tr("column_tooltip_3")));
                for (index, reference) in referenced_columns.iter().enumerate() {
                    tooltip_text.push_str(&format!("<i>\"{}/{}\"</i><br>", reference.0, reference.1));

                    // There is a bug that causes tooltips to be displayed out of screen if they're too big. This fixes it.
                    if index == 50 {
                        tooltip_text.push_str(&format!("<p>{}</p>nnnn", tre("column_tooltip_2", &[&(referenced_columns.len() as isize - 50).to_string()])));
                        break;
                    }
                }

                // Dirty trick to remove the last <br> from the tooltip, or the nnnn in case that text get used.
                tooltip_text.pop();
                tooltip_text.pop();
                tooltip_text.pop();
                tooltip_text.pop();
            }
        }

        // We only add the tooltip if we got something to put into it.
        if !tooltip_text.is_empty() {
            item.set_tool_tip(&QString::from_std_str(&tooltip_text));
        }
    }
}

/// This function returns the reference data for an entire table.
pub unsafe fn get_reference_data(table_name: &str, definition: &Definition) -> Result<BTreeMap<i32, DependencyData>> {

    // Call the backend passing it the files we have open (so we don't get them from the backend too), and get the frontend data while we wait for it to finish.
    let files_to_ignore = UI_STATE.get_open_packedfiles().iter().map(|x| x.get_path()).collect();
    CENTRAL_COMMAND.send_message_qt(Command::GetReferenceDataFromDefinition(table_name.to_owned(), definition.clone(), files_to_ignore));

    let reference_data = definition.get_reference_data();
    let mut dependency_data_visual = BTreeMap::new();

    // If we have a referenced PackedFile open in a view, get the data from the view itself.
    let open_packedfiles = UI_STATE.get_open_packedfiles();
    for (index, (table, column, lookup)) in &reference_data {
        let mut dependency_data_visual_column = BTreeMap::new();
        for packed_file_view in open_packedfiles.iter() {
            let path = packed_file_view.get_ref_path();
            if path.len() == 3 && path[0].to_lowercase() == "db" && path[1].to_lowercase() == format!("{}_tables", table) {
                if let ViewType::Internal(View::Table(table)) = packed_file_view.get_view() {
                    let table = table.get_ref_table();
                    let column = clean_column_names(column);
                    let table_model = &table.table_model;
                    for column_index in 0..table_model.column_count_0a() {
                        if table_model.header_data_2a(column_index, Orientation::Horizontal).to_string().to_std_string() == column {
                            for row in 0..table_model.row_count_0a() {
                                let item = table_model.item_2a(row, column_index);
                                let value = item.text().to_std_string();
                                let lookup_value = match lookup {
                                    Some(columns) => {
                                        let data: Vec<String> = (0..table_model.column_count_0a()).filter(|x| {
                                            columns.contains(&table_model.header_data_2a(*x, Orientation::Horizontal).to_string().to_std_string())
                                        }).map(|x| table_model.item_2a(row, x).text().to_std_string()).collect();
                                        data.join(" ")
                                    },
                                    None => String::new(),
                                };
                                dependency_data_visual_column.insert(value, lookup_value);
                            }
                        }
                    }
                }
            }
        }
        dependency_data_visual.insert(index, dependency_data_visual_column);
    }

    let mut response = CENTRAL_COMMAND.recv_message_qt();
    match response {
        Response::BTreeMapI32DependencyData(ref mut dependency_data) => {
            for index in reference_data.keys() {
                if let Some(mut column_data_visual) = dependency_data_visual.get_mut(index) {
                    if let Some(column_data) = dependency_data.get_mut(index) {
                        column_data.data.append(&mut column_data_visual);
                    }
                }
            }

            Ok(dependency_data.clone())
        },
        Response::Error(error) => Err(error),
        _ => panic!("{}{:?}", THREADS_COMMUNICATION_ERROR, response),
    }
}

/// This function sets up the item delegates for all columns in a table.
pub unsafe fn setup_item_delegates(
    table_view_primary: &QPtr<QTableView>,
    table_view_frozen: &QPtr<QTableView>,
    definition: &Definition,
    dependency_data: &BTreeMap<i32, DependencyData>
) {
    let enable_lookups = false; //table_enable_lookups_button.is_checked();
    for (column, field) in definition.get_fields_processed().iter().enumerate() {

        // Combos are a bit special, as they may or may not replace other delegates. If we disable them, use the normal delegates.
        if !SETTINGS.read().unwrap().settings_bool["disable_combos_on_tables"] && dependency_data.get(&(column as i32)).is_some() || !field.get_enum_values().is_empty() {
            let list = QStringList::new();
            if let Some(data) = dependency_data.get(&(column as i32)) {
                data.data.iter().map(|x| if enable_lookups { x.1 } else { x.0 }).for_each(|x| list.append_q_string(&QString::from_std_str(x)));
            }

            if !field.get_enum_values().is_empty() {
                field.get_enum_values().values().for_each(|x| list.append_q_string(&QString::from_std_str(x)));
            }

            new_combobox_item_delegate_safe(&table_view_primary.static_upcast::<QObject>().as_ptr(), column as i32, list.as_ptr(), true, field.get_max_length());
            new_combobox_item_delegate_safe(&table_view_frozen.static_upcast::<QObject>().as_ptr(), column as i32, list.as_ptr(), true, field.get_max_length());
        }

        else {
            match field.get_ref_field_type() {
                FieldType::Boolean => {},
                FieldType::F32 => {
                    new_doublespinbox_item_delegate_safe(&table_view_primary.static_upcast::<QObject>().as_ptr(), column as i32);
                    new_doublespinbox_item_delegate_safe(&table_view_frozen.static_upcast::<QObject>().as_ptr(), column as i32);
                },
                FieldType::I16 => {
                    new_spinbox_item_delegate_safe(&table_view_primary.static_upcast::<QObject>().as_ptr(), column as i32, 16);
                    new_spinbox_item_delegate_safe(&table_view_frozen.static_upcast::<QObject>().as_ptr(), column as i32, 16);
                },
                FieldType::I32 => {
                    new_spinbox_item_delegate_safe(&table_view_primary.static_upcast::<QObject>().as_ptr(), column as i32, 32);
                    new_spinbox_item_delegate_safe(&table_view_frozen.static_upcast::<QObject>().as_ptr(), column as i32, 32);
                },

                // LongInteger uses normal string controls due to QSpinBox being limited to i32.
                FieldType::I64 => {
                    new_spinbox_item_delegate_safe(&table_view_primary.static_upcast::<QObject>().as_ptr(), column as i32, 64);
                    new_spinbox_item_delegate_safe(&table_view_frozen.static_upcast::<QObject>().as_ptr(), column as i32, 64);
                },
                FieldType::StringU8 |
                FieldType::StringU16 |
                FieldType::OptionalStringU8 |
                FieldType::OptionalStringU16 => {
                    //new_qstring_item_delegate_safe(&mut table_view_primary, column as i32, field.get_max_length());
                    //new_qstring_item_delegate_safe(&mut table_view_frozen, column as i32, field.get_max_length());
                },
                FieldType::SequenceU16(_) | FieldType::SequenceU32(_) => {}
            }
        }
    }
}

/// This function is a generic way to toggle the sort order of a column.
pub unsafe fn sort_column(
    table_view: &QPtr<QTableView>,
    column: i32,
    column_sort_state: Arc<RwLock<(i32, i8)>>
) {
    let mut needs_cleaning = false;
    {
        // We only change the order if it's less than 2. Otherwise, we reset it.
        let mut sort_data = column_sort_state.write().unwrap();
        let mut old_order = if sort_data.0 == column { sort_data.1 } else { 0 };

        if old_order < 2 {
            old_order += 1;
            if old_order == 0 { *sort_data = (-1, old_order); }
            else { *sort_data = (column, old_order); }
        }
        else {
            needs_cleaning = true;
            *sort_data = (-1, -1);
        }
    }

    if needs_cleaning {
        table_view.horizontal_header().set_sort_indicator(-1, SortOrder::AscendingOrder);
    }
}

/// This function is used to build a table struct with the data of a TableView and it's definition.
pub unsafe fn get_table_from_view(
    model: &QPtr<QStandardItemModel>,
    definition: &Definition
) -> Result<Table> {
    let mut entries = vec![];

    for row in 0..model.row_count_0a() {
        let mut new_row: Vec<DecodedData> = vec![];

        // Bitwise columns can span across multiple columns. That means we have to keep track of the column ourselfs.
        for (column, field) in definition.get_fields_processed().iter().enumerate() {

            // Create a new Item.
            let item = match field.get_ref_field_type() {

                // This one needs a couple of changes before turning it into an item in the table.
                FieldType::Boolean => DecodedData::Boolean(model.item_2a(row as i32, column as i32).check_state() == CheckState::Checked),

                // Numbers need parsing, and this can fail.
                FieldType::F32 => DecodedData::F32(model.item_2a(row as i32, column as i32).data_1a(2).to_float_0a()),
                FieldType::I16 => DecodedData::I16(model.item_2a(row as i32, column as i32).data_1a(2).to_int_0a() as i16),
                FieldType::I32 => DecodedData::I32(model.item_2a(row as i32, column as i32).data_1a(2).to_int_0a()),
                FieldType::I64 => DecodedData::I64(model.item_2a(row as i32, column as i32).data_1a(2).to_long_long_0a()),

                // All these are just normal Strings.
                FieldType::StringU8 => DecodedData::StringU8(QString::to_std_string(&model.item_2a(row as i32, column as i32).text())),
                FieldType::StringU16 => DecodedData::StringU16(QString::to_std_string(&model.item_2a(row as i32, column as i32).text())),
                FieldType::OptionalStringU8 => DecodedData::OptionalStringU8(QString::to_std_string(&model.item_2a(row as i32, column as i32).text())),
                FieldType::OptionalStringU16 => DecodedData::OptionalStringU16(QString::to_std_string(&model.item_2a(row as i32, column as i32).text())),

                // Sequences in the UI are not yet supported.
                FieldType::SequenceU16(_) => DecodedData::SequenceU16(serde_json::from_str(&model.item_2a(row as i32, column as i32).data_1a(ITEM_SEQUENCE_DATA).to_string().to_std_string()).unwrap()),
                FieldType::SequenceU32(_) => DecodedData::SequenceU32(serde_json::from_str(&model.item_2a(row as i32, column as i32).data_1a(ITEM_SEQUENCE_DATA).to_string().to_std_string()).unwrap()),
            };
            new_row.push(item);
        }
        entries.push(new_row);
    }

    let mut table = Table::new(definition);
    table.set_table_data(&entries)?;
    Ok(table)
}

/// This function creates a new subtable from the current table.
pub unsafe fn open_subtable(
    parent: QPtr<QWidget>,
    app_ui: &Rc<AppUI>,
    global_search_ui: &Rc<GlobalSearchUI>,
    pack_file_contents_ui: &Rc<PackFileContentsUI>,
    diagnostics_ui: &Rc<DiagnosticsUI>,
    table_data: TableType,
) -> Option<String> {

    // Create and configure the dialog.
    let dialog = QDialog::new_1a(parent);
    dialog.set_window_title(&qtr("nested_table_title"));
    dialog.set_modal(true);
    dialog.resize_2a(600, 200);

    let main_grid = create_grid_layout(dialog.static_upcast());
    let main_widget = QWidget::new_1a(&dialog);
    let _widget_grid = create_grid_layout(main_widget.static_upcast());
    let accept_button = QPushButton::from_q_string(&qtr("nested_table_accept"));

    let table_view = TableView::new_view(&main_widget, app_ui, global_search_ui, pack_file_contents_ui, diagnostics_ui, table_data, None).unwrap();

    main_grid.add_widget_5a(&main_widget, 0, 0, 1, 1);
    main_grid.add_widget_5a(&accept_button, 1, 0, 1, 1);

    accept_button.released().connect(dialog.slot_accept());

    if dialog.exec() == 1 {
        if let Ok(table) = get_table_from_view(&table_view.table_model.static_upcast(), &table_view.get_ref_table_definition()) {
            Some(serde_json::to_string(&table).unwrap())
        } else {
            show_dialog(&table_view.table_view_primary, ErrorKind::Generic, false);
            None
        }
    } else { None }
}

/// This function returns the field list of a table, properly sorted for the view.
pub fn get_fields_sorted(table_definition: &Definition) -> Vec<Field> {
    let mut fields = table_definition.get_fields_processed().to_vec();
    fields.sort_by(|a, b| {
        if SETTINGS.read().unwrap().settings_bool["tables_use_old_column_order"] {
            if a.get_is_key() && b.get_is_key() { Ordering::Equal }
            else if a.get_is_key() && !b.get_is_key() { Ordering::Less }
            else if !a.get_is_key() && b.get_is_key() { Ordering::Greater }
            else { Ordering::Equal }
        }
        else if a.get_ca_order() == -1 || b.get_ca_order() == -1 { Ordering::Equal }
        else { a.get_ca_order().cmp(&b.get_ca_order()) }
    });
    fields
}
