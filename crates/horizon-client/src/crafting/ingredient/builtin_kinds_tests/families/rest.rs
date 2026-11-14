use super::*;

#[test]
#[ignore = "requires prepared resources"]
fn components_removal_requires_absence_including_defaults() {
    let items = resources();
    let ingredient = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"!minecraft:enchantments":{}}}"#,
        &items,
    );
    assert!(!ingredient.shown.as_ref().unwrap()[0].carries_component(
        items.get("minecraft:diamond_sword"),
        "minecraft:enchantments"
    ));
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let mut sword = ItemStack::new("minecraft:diamond_sword", 1);
    assert!(!ingredient.test_in(&context, &sword));
    let patch = Tag::compound([("!minecraft:enchantments", Tag::Compound(Default::default()))]);
    (_, sword.extra) = crate::item_data::decode_components(Some(&patch));
    assert!(ingredient.test_in(&context, &sword));
    sword = ItemStack::new("minecraft:diamond_sword", 1);
    sword.data_mut().set_loot_component(
        "minecraft:enchantments",
        &Some(Tag::Compound(Default::default())),
    );
    assert!(!ingredient.test_in(&context, &sword));
}
#[test]
#[ignore = "requires prepared resources"]
fn block_tag_logs_uses_block_items_and_rejects_hash() {
    let items = resources();
    let ingredient = parse(
        r#"{"type":"neoforge:block_tag","tag":"minecraft:logs"}"#,
        &items,
    );
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    assert!(ingredient.test_in(&context, &ItemStack::new("minecraft:oak_log", 1)));
    assert!(!ingredient.test_in(&context, &ItemStack::new("minecraft:stone", 1)));
    assert!(ingredient.is_simple());
    let mut alias_items = items.clone();
    alias_items.block_tag_order.insert(
        "fixture:wall_signs".into(),
        vec!["minecraft:oak_wall_sign".into()],
    );
    let wall = parse(
        r#"{"type":"neoforge:block_tag","tag":"fixture:wall_signs"}"#,
        &alias_items,
    );
    assert_eq!(wall.order, ["minecraft:oak_sign"]);
    assert!(
        Ingredient::parse(
            &serde_json::json!({"type":"neoforge:block_tag","tag":"#minecraft:logs"}),
            &items
        )
        .is_none()
    );
}
#[test]
fn custom_display_matches_base_but_shows_another_item() {
    let items = Items::empty();
    let ingredient = parse(
        r#"{"type":"neoforge:custom_display","base":"minecraft:apple","display":{"type":"minecraft:item","item":"minecraft:diamond"}}"#,
        &items,
    );
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    assert!(ingredient.test_in(&context, &ItemStack::new("minecraft:apple", 1)));
    assert!(!ingredient.test_in(&context, &ItemStack::new("minecraft:diamond", 1)));
    assert!(ingredient.is_simple());
    assert_eq!(
        crate::recipe_book::Book::default().items_of(&ingredient),
        ["minecraft:diamond"]
    );
    assert_eq!(ingredient.order, ["minecraft:apple"]);
    let placement = crate::crafting::Placement::of(vec![Some(ingredient)], None);
    assert_eq!(placement.ingredients[0].order, ["minecraft:apple"]);
    // 26.3 TagSlotDisplay carries an item HolderSet, including the # prefix.
    let tags = Items::for_tests(std::collections::HashMap::from([(
        "minecraft:planks".into(),
        std::collections::HashSet::from(["minecraft:oak_planks".into()]),
    )]));
    let tagged = parse(
        r##"{"type":"neoforge:custom_display","base":"minecraft:apple","display":{"type":"minecraft:tag","tag":"#minecraft:planks"}}"##,
        &tags,
    );
    assert_eq!(
        crate::recipe_book::Book::default().items_of(&tagged),
        ["minecraft:oak_planks"]
    );
}
#[test]
fn simplicity_follows_all_children_and_custom_display_base() {
    let items = Items::empty();
    for source in [
        r#"{"type":"neoforge:compound","children":["minecraft:apple","minecraft:stone"]}"#,
        r#"{"type":"neoforge:intersection","children":["minecraft:apple","minecraft:apple"]}"#,
        r#"{"type":"neoforge:difference","base":["minecraft:apple","minecraft:stone"],"subtracted":"minecraft:stone"}"#,
    ] {
        assert!(parse(source, &items).is_simple());
    }
    let source = r#"{"type":"neoforge:custom_display","base":{"type":"neoforge:components","items":"minecraft:apple","components":{}},"display":{"type":"minecraft:item","item":"minecraft:diamond"}}"#;
    assert!(!parse(source, &items).is_simple());
    let mut custom = Ingredient::of(vec!["minecraft:apple".into()]);
    custom.custom = Some(("fixture:custom".into(), Default::default()));
    assert!(!custom.is_simple());
}

#[test]
fn builtin_kinds_bypass_mod_resolution_and_honor_type_key_priority() {
    let load = load();
    for source in [
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5}}"#,
        r#"{"type":"neoforge:block_tag","tag":"minecraft:logs"}"#,
        r#"{"type":"fixture:wrong","neoforge:ingredient_type":"neoforge:custom_display","base":"minecraft:apple","display":{"type":"minecraft:item","item":"minecraft:diamond"}}"#,
    ] {
        let mut json: Value = serde_json::from_str(source).unwrap();
        let original = json.clone();
        assert!(!Ingredient::resolve(&mut json, &load));
        assert_eq!(json, original);
    }
}

#[test]
fn recipe_book_and_placement_use_component_ingredient_item_lists() {
    use crate::recipe_book::{Book, Category, Display, Entry};
    let items = Items::empty();
    let ingredient = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:damage":5}}"#,
        &items,
    );
    let entry = Entry {
        id: "fixture:components".into(),
        display_id: -1,
        category: Category::CraftingEquipment,
        group: None,
        display: Display::Shapeless(vec![ingredient.clone()]),
        result: ItemStack::new("minecraft:diamond", 1),
        variants: Vec::new(),
        stacks: Vec::new(),
    };
    let mut book = Book::default();
    let placement = crate::crafting::Placement::of(vec![Some(ingredient)], None);
    book.placements.insert(entry.id.clone(), placement.clone());
    let mut stock = crate::menu::place::Stacked::default();
    // An undamaged sword cannot satisfy the concrete recipe, but the book and
    // placement intentionally account only for its item holder.
    stock.account(&std::sync::Arc::from("minecraft:diamond_sword"), 1);
    assert!(book.can_craft(&entry, &stock));
    assert!(stock.can_craft(&placement.ingredients.iter().collect::<Vec<_>>()));
    assert_eq!(
        book.items_of(&placement.ingredients[0]),
        ["minecraft:diamond_sword"]
    );
}

#[test]
fn components_enchantable_decodes_positive_records_and_matches_values() {
    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:diamond_sword".into(),
        horizon_core::nbt::Compound::from([(
            "minecraft:enchantable".into(),
            Tag::compound([
                ("value", Tag::Short(15)),
                ("ignored", Tag::String("prototype".into())),
            ]),
        )]),
    )]));
    let ingredient = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:enchantable":{"value":15}}}"#,
        &items,
    );
    let normalized = horizon_command::item::decoded_component_value(
        "minecraft:enchantable",
        &Tag::compound([
            ("value", Tag::Double(15.9f64.to_bits())),
            ("ignored", Tag::String("not part of the record".into())),
        ]),
        &items.component_registry,
    )
    .unwrap();
    assert_eq!(normalized, Tag::compound([("value", Tag::Int(15))]));

    for source in [
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:enchantable":{"value":0}}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:enchantable":{"value":-1}}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:enchantable":{}}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:enchantable":{"value":"15"}}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:enchantable":15}}"#,
    ] {
        let json: Value = serde_json::from_str(source).unwrap();
        assert!(Ingredient::parse(&json, &items).is_none(), "{source}");
        let rule = IngredientRule::Components {
            patch: crate::json_nbt::json_to_compound(json["components"].as_object().unwrap())
                .unwrap(),
            removed: Vec::new(),
            strict: false,
        };
        let error = components::validate(&rule, &items).unwrap_err();
        assert!(error.contains("minecraft:enchantable"), "{error}");
    }

    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let mut sword = ItemStack::new("minecraft:diamond_sword", 1);
    assert!(
        ingredient.test_in(&context, &sword),
        "borrowed item default"
    );
    for value in [
        Tag::Byte(15),
        Tag::Short(15),
        Tag::Int(15),
        Tag::Long(4_294_967_311),
        Tag::Float(15.9f32.to_bits()),
        Tag::Double(15.9f64.to_bits()),
    ] {
        sword.data_mut().set_other(
            "minecraft:enchantable",
            Some(Tag::compound([
                ("value", value),
                ("ignored", Tag::String("live".into())),
            ])),
        );
        assert!(ingredient.test_in(&context, &sword));
    }
    sword.data_mut().set_other(
        "minecraft:enchantable",
        Some(Tag::compound([("value", Tag::Int(16))])),
    );
    assert!(!ingredient.test_in(&context, &sword));
    sword.data_mut().set_other(
        "minecraft:enchantable",
        Some(Tag::compound([("value", Tag::Int(0))])),
    );
    assert!(!ingredient.test_in(&context, &sword));

    let absence_source = r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"!minecraft:enchantable":{}}}"#;
    let absence = parse(absence_source, &items);
    let mut removed = ItemStack::new("minecraft:diamond_sword", 1);
    assert!(!absence.test_in(&context, &removed));
    removed.data_mut().set_other(
        "!minecraft:enchantable",
        Some(Tag::compound([] as [(&str, Tag); 0])),
    );
    assert!(absence.test_in(&context, &removed));
    assert!(!ingredient.test_in(&context, &removed));
    let no_defaults = Items::empty();
    let bare = ItemStack::new("minecraft:diamond_sword", 1);
    assert!(parse(absence_source, &no_defaults).test_in(&context, &bare));
    let positive_without_default = parse(
        r#"{"type":"neoforge:components","items":"minecraft:diamond_sword","components":{"minecraft:enchantable":{"value":15}}}"#,
        &no_defaults,
    );
    assert!(!positive_without_default.test_in(&context, &bare));
}

#[test]
fn item_model_component_literals_decode_validate_and_match() {
    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:stick".into(),
        crate::json_nbt::json_to_compound(
            serde_json::json!({"minecraft:item_model":"unregistered/model"})
                .as_object()
                .unwrap(),
        )
        .unwrap(),
    )]));
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    for source in [
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"unregistered/model"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"minecraft:unregistered/model"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":":unregistered/model"}}"#,
    ] {
        let ingredient = parse(source, &items);
        assert_eq!(
            ingredient.shown.as_ref().unwrap()[0]
                .data()
                .unwrap()
                .other("minecraft:item_model"),
            Some(&Tag::String("minecraft:unregistered/model".into())),
        );
        let mut stack = ItemStack::new("minecraft:stick", 1);
        let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
            assert!(ingredient.test_in(&context, &stack));
        });
        assert_eq!(allocations.calls, 0, "{allocations:?}");
        for model in [
            "unregistered/model",
            "minecraft:unregistered/model",
            ":unregistered/model",
        ] {
            stack
                .data_mut()
                .set_loot_component("minecraft:item_model", &Some(Tag::String(model.into())));
            let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
                assert!(ingredient.test_in(&context, &stack));
            });
            assert_eq!(allocations.calls, 0, "{allocations:?}");
        }
        for model in [
            "minecraft:other",
            "fixture:unregistered/model",
            "Minecraft:unregistered/model",
        ] {
            stack
                .data_mut()
                .set_loot_component("minecraft:item_model", &Some(Tag::String(model.into())));
            assert!(!ingredient.test_in(&context, &stack));
        }
    }
    for source in [
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"Minecraft:stick"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"minecraft:Stick"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"bad namespace:stick"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"..:stick"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":"a:b:c"}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:stick","components":{"minecraft:item_model":5}}"#,
    ] {
        let value: serde_json::Value = serde_json::from_str(source).unwrap();
        assert!(Ingredient::parse(&value, &items).is_none());
        let rule = IngredientRule::Components {
            patch: crate::json_nbt::json_to_compound(value["components"].as_object().unwrap())
                .unwrap(),
            removed: Vec::new(),
            strict: false,
        };
        let error = components::validate(&rule, &items).unwrap_err();
        assert!(error.contains("component minecraft:item_model:"), "{error}");
    }
    for model in ["", ":", "minecraft:"] {
        assert_eq!(
            horizon_command::item::decoded_component_value(
                "minecraft:item_model",
                &Tag::String(model.into()),
                &items.component_registry,
            )
            .unwrap(),
            Tag::String("minecraft:".into()),
        );
    }
    for units in [vec![0xd800], vec![0xdc00], vec![0xd83d, 0xde00]] {
        let invalid_utf16 = Tag::String(horizon_core::nbt::JavaString(units));
        assert!(
            horizon_command::item::decoded_component_value(
                "minecraft:item_model",
                &invalid_utf16,
                &items.component_registry,
            )
            .is_err()
        );
    }
    let absence = parse(
        r#"{"type":"neoforge:components","items":["minecraft:stick","minecraft:iron_ingot"],"components":{"!minecraft:item_model":{}}}"#,
        &items,
    );
    let positive = parse(
        r#"{"type":"neoforge:components","items":["minecraft:stick","minecraft:iron_ingot"],"components":{"minecraft:item_model":"unregistered/model"}}"#,
        &items,
    );
    let mut removed = ItemStack::new("minecraft:stick", 1);
    assert!(!absence.test_in(&context, &removed));
    removed.data_mut().set_loot_component(
        "!minecraft:item_model",
        &Some(Tag::Compound(Default::default())),
    );
    assert!(absence.test_in(&context, &removed));
    assert!(!positive.test_in(&context, &removed));
    let mut populated = ItemStack::new("minecraft:stick", 1);
    populated.data_mut().set_loot_component(
        "minecraft:item_model",
        &Some(Tag::String("unregistered/model".into())),
    );
    assert!(!absence.test_in(&context, &populated));
    let no_default = ItemStack::new("minecraft:iron_ingot", 1);
    assert!(absence.test_in(&context, &no_default));
    assert!(!positive.test_in(&context, &no_default));
}

#[test]
fn enchantment_glint_override_decodes_boolean_literals_and_borrowed_numeric_values() {
    const COMPONENT: &str = "minecraft:enchantment_glint_override";
    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:apple".into(),
        std::collections::BTreeMap::from([(COMPONENT.into(), Tag::Short(256))]),
    )]));
    let enabled = parse(
        r#"{"type":"neoforge:components","items":"minecraft:apple","components":{"minecraft:enchantment_glint_override":true}}"#,
        &items,
    );
    let disabled = parse(
        r#"{"type":"neoforge:components","items":"minecraft:apple","components":{"minecraft:enchantment_glint_override":false}}"#,
        &items,
    );
    for (ingredient, expected) in [(&enabled, Tag::Byte(1)), (&disabled, Tag::Byte(0))] {
        assert_eq!(
            ingredient.shown.as_ref().unwrap()[0]
                .data()
                .unwrap()
                .other(COMPONENT),
            Some(&expected),
        );
    }
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let mut candidate = ItemStack::new("minecraft:apple", 1);
    assert!(
        enabled.test_in(&context, &candidate),
        "inherited numeric default"
    );
    assert!(!disabled.test_in(&context, &candidate));

    // NbtOps.getBooleanValue uses Number.doubleValue, not Tag.asBoolean's byteValue.
    for (value, expected) in [
        (Tag::Byte(-1), true),
        (Tag::Short(256), true),
        (Tag::Int(256), true),
        (Tag::Long(4_294_967_296), true),
        (Tag::Float(0.5_f32.to_bits()), true),
        (Tag::Double(0.5_f64.to_bits()), true),
        (Tag::Float(f32::NAN.to_bits()), true),
        (Tag::Double(f64::NAN.to_bits()), true),
        (Tag::Double(f64::INFINITY.to_bits()), true),
        (Tag::Double(f64::from_bits(1).to_bits()), true),
        (Tag::Byte(0), false),
        (Tag::Short(0), false),
        (Tag::Int(0), false),
        (Tag::Long(0), false),
        (Tag::Float((-0.0_f32).to_bits()), false),
        (Tag::Double((-0.0_f64).to_bits()), false),
    ] {
        assert_eq!(
            horizon_command::item::decoded_component_value(
                COMPONENT,
                &value,
                &items.component_registry,
            )
            .unwrap(),
            Tag::Byte(i8::from(expected)),
        );
        candidate
            .data_mut()
            .set_loot_component(COMPONENT, &Some(value));
        assert_eq!(enabled.test_in(&context, &candidate), expected);
        assert_eq!(disabled.test_in(&context, &candidate), !expected);
    }
    let numeric = parse(
        r#"{"type":"neoforge:components","items":"minecraft:apple","components":{"minecraft:enchantment_glint_override":0.5}}"#,
        &items,
    );
    candidate
        .data_mut()
        .set_loot_component(COMPONENT, &Some(Tag::Int(256)));
    assert!(numeric.test_in(&context, &candidate));
    let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
        for _ in 0..100 {
            assert!(enabled.test_in(&context, std::hint::black_box(&candidate)));
        }
    });
    assert_eq!(allocations.calls, 0, "{allocations:?}");
    assert_eq!(allocations.bytes, 0);
    candidate.data_mut().set_loot_component(COMPONENT, &None);
    assert!(
        enabled.test_in(&context, &candidate),
        "default is inherited again"
    );
    candidate.data_mut().set_loot_component(
        "!minecraft:enchantment_glint_override",
        &Some(Tag::Compound(Default::default())),
    );
    assert!(
        !enabled.test_in(&context, &candidate),
        "removed, not inherited"
    );
    assert!(
        !disabled.test_in(&context, &candidate),
        "absent is not false"
    );

    let invalid: Value = serde_json::from_str(
        r#"{"type":"neoforge:components","items":"minecraft:apple","components":{"minecraft:enchantment_glint_override":"true"}}"#,
    )
    .unwrap();
    assert!(Ingredient::parse(&invalid, &items).is_none());
    let rule = IngredientRule::Components {
        patch: crate::json_nbt::json_to_compound(invalid["components"].as_object().unwrap())
            .unwrap(),
        removed: Vec::new(),
        strict: false,
    };
    assert_eq!(
        components::validate(&rule, &items).unwrap_err(),
        "component minecraft:enchantment_glint_override: Not a number",
    );
    let absence = parse(
        r#"{"type":"neoforge:components","items":["minecraft:apple","minecraft:stick"],"components":{"!minecraft:enchantment_glint_override":{}}}"#,
        &items,
    );
    let mut removed = ItemStack::new("minecraft:apple", 1);
    assert!(!absence.test_in(&context, &removed));
    removed.data_mut().set_loot_component(
        "!minecraft:enchantment_glint_override",
        &Some(Tag::Compound(Default::default())),
    );
    assert!(absence.test_in(&context, &removed));
    assert!(!enabled.test_in(&context, &removed));
    assert!(!disabled.test_in(&context, &removed));
    let mut populated = ItemStack::new("minecraft:apple", 1);
    populated
        .data_mut()
        .set_loot_component(COMPONENT, &Some(Tag::Byte(0)));
    assert!(!absence.test_in(&context, &populated));
    assert!(absence.test_in(&context, &ItemStack::new("minecraft:stick", 1)));
}
