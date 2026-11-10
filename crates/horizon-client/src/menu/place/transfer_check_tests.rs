//! A viewer transfer tried and undone (`Menu::check_transfer`), against the
//! vanilla data the menu tests use.
use crate::menu::{
    Kind, Menu,
    place::{TransferCheck, TransferError},
    tests::{mod_crafting_table, stack, with_menu},
};

/// The check answers as the transfer would, names the cells nothing fills,
/// counts a mod grid's sources, and moves nothing.
#[test]
fn a_check_answers_as_the_transfer_and_moves_nothing() {
    let missing = |cells: Vec<usize>| TransferCheck {
        result: Err(TransferError::Missing),
        missing: cells,
    };
    with_menu(Kind::Crafting, |env, menu| {
        // A stick is two planks, one above the other.
        assert_eq!(menu.check_transfer(env, "minecraft:stick"), missing(vec![0, 1]));
        menu.set_player_item(4, stack("oak_planks", 1));
        assert_eq!(menu.check_transfer(env, "minecraft:stick"), missing(vec![1]));
        menu.set_player_item(5, stack("birch_planks", 1));
        let ok = menu.check_transfer(env, "minecraft:stick");
        assert_eq!(ok.result, Ok(()));
        assert!(ok.missing.is_empty());
        // Nothing moved.
        assert_eq!(menu.player_item(4), stack("oak_planks", 1).as_ref());
        assert!((0..10).all(|s| menu.slot(s).is_none()));
        // A crafting table is a 2x2 of planks: the last two lack.
        assert_eq!(
            menu.check_transfer(env, "minecraft:crafting_table"),
            missing(vec![2, 3])
        );
        assert_eq!(
            menu.check_transfer(env, "minecraft:no_such_recipe").result,
            Err(TransferError::Unavailable)
        );
    });
    let station = &mut Menu::new(mod_crafting_table());
    with_menu(Kind::Inventory, |env, _| {
        station.set_player_item(0, stack("oak_planks", 1));
        station.sources = vec![vec![None, stack("oak_planks", 3)]];
        let before = station.sources.clone();
        assert_eq!(
            station.check_transfer(env, "minecraft:crafting_table").result,
            Ok(())
        );
        assert_eq!(station.sources, before);
        assert_eq!(station.player_item(0), stack("oak_planks", 1).as_ref());
        station.sources = vec![vec![stack("dirt", 5)]];
        assert_eq!(
            station.check_transfer(env, "minecraft:stick"),
            missing(vec![1])
        );
    });
}
