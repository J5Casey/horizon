use super::*;

#[test]
fn components_dyed_color_decodes_rgb_forms_and_matches_borrowed_values() {
    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:leather_helmet".into(),
        crate::json_nbt::json_to_compound(
            serde_json::json!({"minecraft:dyed_color":[1,0,0]})
                .as_object()
                .unwrap(),
        )
        .unwrap(),
    )]));
    let integer = parse(
        r#"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":-65536}}"#,
        &items,
    );
    let vector = parse(
        r#"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":[1.0,0.0,0.0]}}"#,
        &items,
    );
    assert_eq!(
        vector.shown.as_ref().unwrap()[0].data().unwrap().dyed_color,
        Some(-65536)
    );
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let default = ItemStack::new("minecraft:leather_helmet", 1);
    let mut typed = default.clone();
    typed.data_mut().dyed_color = Some(-65536);
    let mut raw = default.clone();
    raw.data_mut().set_other(
        "minecraft:dyed_color",
        Some(Tag::List(vec![Tag::Int(1), Tag::Int(0), Tag::Int(0)])),
    );
    for ingredient in [&integer, &vector] {
        assert!(ingredient.test_in(&context, &default));
        assert!(ingredient.test_in(&context, &typed));
        assert!(ingredient.test_in(&context, &raw));
    }
    let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
        for _ in 0..100 {
            for ingredient in [&integer, &vector] {
                for stack in [&default, &typed, &raw] {
                    assert!(std::hint::black_box(ingredient.test_in(&context, stack)));
                }
            }
        }
    });
    assert_eq!(allocations.calls, 0, "{allocations:?}");
    assert_eq!(allocations.bytes, 0);
    // RGB equality retains the integer's alpha bits; rendering opacity is unrelated.
    typed.data_mut().dyed_color = Some(16711680);
    assert!(!integer.test_in(&context, &typed));
    typed.data_mut().dyed_color = Some(-16776961);
    assert!(!vector.test_in(&context, &typed));
    raw.data_mut().set_other(
        "minecraft:dyed_color",
        Some(Tag::String("not a color".into())),
    );
    assert!(!integer.test_in(&context, &raw));

    // All boxed number widths and NbtOps collection forms reach the same codec.
    for (value, expected) in [
        (Tag::Byte(-1), -1),
        (Tag::Short(-32768), -32768),
        (Tag::Int(305419896), 305419896),
        (Tag::Long(4294901760), -65536),
        (Tag::Float((-65536.5f32).to_bits()), -65536),
        (Tag::Double((-65536.5f64).to_bits()), -65536),
        (Tag::ByteArray(vec![1, 0, 0]), -65536),
        (Tag::IntArray(vec![1, 0, 0]), -65536),
        (Tag::LongArray(vec![1, 0, 0]), -65536),
        (
            Tag::List(vec![
                Tag::Double(0.5f64.to_bits()),
                Tag::Double(0.0f64.to_bits()),
                Tag::Double(0.0f64.to_bits()),
            ]),
            -8454144,
        ),
        // Channels have no 0..1 restriction, and floor before byte masking.
        (Tag::ByteArray(vec![255, 2, 0]), 0xff01_fe00u32 as i32),
        (
            Tag::List(vec![
                Tag::Float(f32::NAN.to_bits()),
                Tag::Float(f32::INFINITY.to_bits()),
                Tag::Float(f32::NEG_INFINITY.to_bits()),
            ]),
            0xff00_ff00u32 as i32,
        ),
    ] {
        assert_eq!(
            horizon_command::item::decoded_component_value(
                "minecraft:dyed_color",
                &value,
                &items.component_registry,
            )
            .unwrap(),
            Tag::Int(expected)
        );
        let patch = Tag::compound([("minecraft:dyed_color", value)]);
        let (_, data) = crate::item_data::decode_components(Some(&patch));
        assert_eq!(data.unwrap().dyed_color, Some(expected));
    }

    for source in [
        r##"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":"#ff0000"}}"##,
        r#"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":[1,0]}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":[1,0,0,1]}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":[1,"red",0]}}"#,
        r#"{"type":"neoforge:components","items":"minecraft:leather_helmet","components":{"minecraft:dyed_color":{"rgb":16711680,"show_in_tooltip":true}}}"#,
    ] {
        let json: serde_json::Value = serde_json::from_str(source).unwrap();
        assert!(Ingredient::parse(&json, &items).is_none());
        let rule = IngredientRule::Components {
            patch: crate::json_nbt::json_to_compound(json["components"].as_object().unwrap())
                .unwrap(),
            removed: Vec::new(),
            strict: false,
        };
        let error = components::validate(&rule, &items).unwrap_err();
        assert!(error.contains("minecraft:dyed_color"), "{error}");
    }
    let absence = parse(
        r#"{"type":"neoforge:components","items":["minecraft:leather_helmet","minecraft:stick"],"components":{"!minecraft:dyed_color":{}}}"#,
        &items,
    );
    assert!(!absence.test_in(&context, &default));
    let mut populated_typed = default.clone();
    populated_typed.data_mut().dyed_color = Some(-65536);
    assert!(!absence.test_in(&context, &populated_typed));
    let mut populated_raw = default.clone();
    populated_raw.data_mut().set_other(
        "minecraft:dyed_color",
        Some(Tag::List(vec![Tag::Int(1), Tag::Int(0), Tag::Int(0)])),
    );
    assert!(!absence.test_in(&context, &populated_raw));
    let mut removed = default.clone();
    removed.data_mut().set_loot_component(
        "!minecraft:dyed_color",
        &Some(Tag::Compound(Default::default())),
    );
    assert!(absence.test_in(&context, &removed));
    assert!(!integer.test_in(&context, &removed));
    assert!(!vector.test_in(&context, &removed));
    assert!(absence.test_in(&context, &ItemStack::new("minecraft:stick", 1)));
}

#[test]
fn dye_components_compile_recipes_match_live_values_and_preserve_defaults() {
    let mut items = Items::empty();
    items.component_defaults = std::sync::Arc::new(std::collections::HashMap::from([(
        "minecraft:red_dye".into(),
        crate::json_nbt::json_to_compound(
            serde_json::json!({"minecraft:dye":"red"})
                .as_object()
                .unwrap(),
        )
        .unwrap(),
    )]));
    let ingredient = parse(
        r#"{"type":"neoforge:components","items":"minecraft:red_dye","components":{"minecraft:dye":"red"},"strict":true}"#,
        &items,
    );
    let load = load();
    let context = load.loop_context(LoopSide::Server, []);
    let root = tempfile::tempdir().unwrap();
    let recipes = crate::crafting::Recipes::from_sources(
        root.path(),
        &items,
        &horizon_world::loot::PackOverlay::NONE,
        vec![(
            "fixture:dye".into(),
            serde_json::json!({
                "type":"minecraft:crafting_shapeless",
                "ingredients":[{
                    "type":"neoforge:components", "items":"minecraft:red_dye",
                    "components":{"minecraft:dye":"red"}, "strict":true
                }],
                "result":{"id":"minecraft:diamond"}
            }),
        )],
        &load,
    )
    .unwrap();
    assert_eq!(recipes.crafting.len(), 1);
    let mut stack = ItemStack::new("minecraft:red_dye", 1);
    assert!(
        ingredient.test_in(&context, &stack),
        "prototype dye default"
    );
    assert!(recipes.craft(&context, &[Some(stack.clone())], 1).is_some());
    stack
        .data_mut()
        .set_loot_component("minecraft:dye", &Some(Tag::String("red".into())));
    assert_eq!(
        stack.data().unwrap().other("minecraft:dye"),
        Some(&Tag::String("red".into()))
    );
    assert!(ingredient.test_in(&context, &stack));
    let (_, allocations) = horizon_threads::allocation_probe::measure(|| {
        for _ in 0..100 {
            assert!(ingredient.test_in(&context, std::hint::black_box(&stack)));
        }
    });
    assert_eq!(allocations.calls, 0, "{allocations:?}");
    assert_eq!(allocations.bytes, 0);
    for value in [
        Tag::String("blue".into()),
        Tag::String("RED".into()),
        Tag::Int(14),
    ] {
        stack
            .data_mut()
            .set_loot_component("minecraft:dye", &Some(value));
        assert!(!ingredient.test_in(&context, &stack));
        assert!(recipes.craft(&context, &[Some(stack.clone())], 1).is_none());
    }
    stack.data_mut().set_loot_component("minecraft:dye", &None);
    stack
        .data_mut()
        .set_loot_component("!minecraft:dye", &Some(Tag::Compound(Default::default())));
    assert!(!ingredient.test_in(&context, &stack));
    let absent = parse(
        r#"{"type":"neoforge:components","items":"minecraft:red_dye","components":{"!minecraft:dye":{}}}"#,
        &items,
    );
    assert!(absent.test_in(&context, &stack));
    assert!(!absent.test_in(&context, &ItemStack::new("minecraft:red_dye", 1)));
    assert!(!ingredient.test_in(&context, &ItemStack::new("minecraft:blue_dye", 1)));
    let stone_absent = parse(
        r#"{"type":"neoforge:components","items":"minecraft:stone","components":{"!minecraft:dye":{}}}"#,
        &items,
    );
    assert!(stone_absent.test_in(&context, &ItemStack::new("minecraft:stone", 1)));
}

#[test]
fn malformed_dye_payloads_reject_whole_recipes_with_component_errors() {
    let items = Items::empty();
    let load = load();
    let root = tempfile::tempdir().unwrap();
    for value in [
        serde_json::json!("RED"),
        serde_json::json!("silver"),
        serde_json::json!("minecraft:red"),
        serde_json::json!(14),
        serde_json::json!(true),
        serde_json::json!(false),
        serde_json::Value::Null,
        serde_json::json!({}),
        serde_json::json!([]),
    ] {
        let patch = serde_json::json!({"minecraft:dye":value});
        if !value.is_null() {
            let rule = IngredientRule::Components {
                patch: crate::json_nbt::json_to_compound(patch.as_object().unwrap()).unwrap(),
                removed: vec![],
                strict: false,
            };
            let error = components::validate(&rule, &items).unwrap_err();
            assert!(error.contains("component minecraft:dye:"), "{error}");
        }
        let ingredient = serde_json::json!({
            "type":"neoforge:components", "items":"minecraft:red_dye", "components":patch
        });
        assert!(Ingredient::parse(&ingredient, &items).is_none());
        let recipes = crate::crafting::Recipes::from_sources(
            root.path(),
            &items,
            &horizon_world::loot::PackOverlay::NONE,
            vec![(
                "fixture:invalid_dye".into(),
                serde_json::json!({
                    "type":"minecraft:crafting_shapeless", "ingredients":[ingredient],
                    "result":{"id":"minecraft:diamond"}
                }),
            )],
            &load,
        )
        .unwrap();
        assert!(recipes.crafting.is_empty());
        assert!(recipes.by_type.values().all(|recipes| recipes.is_empty()));
    }
}
