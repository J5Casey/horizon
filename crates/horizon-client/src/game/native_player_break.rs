//! A player's break for native mods: what a vein miner needs to break the
//! blocks around the one its player broke as that player breaks them
//! (docs/native-mods.md, "Breaking as a player").
use super::Game;
use horizon_client::{items::ItemStack, mod_api::NativeBreakDrops};

/// What a player's break keeps instead of popping (`Game::break_collect`):
/// the block's own drops, and its experience when `experience` is Some.
#[derive(Default)]
pub(super) struct BreakKept {
    pub(super) stacks: Vec<ItemStack>,
    pub(super) experience: Option<u32>,
}

impl Game<'_> {
    /// Breaks the block at `pos` as the player `uuid` breaks it, through the
    /// game's own `ServerPlayerGameMode.destroyBlock`, as FTB Ultimine does:
    /// a spectator, an adventure player whose tool can't break it, a held item
    /// that can't break blocks (a sword in creative) and a game master block
    /// without the permission leave it, and so does spawn protection on a
    /// dedicated server. Then the mods' `native_destroy_block` hears it (and
    /// can cancel it), and the break runs as a player's: the block's own
    /// effects, its events, particles and sound, the drops rolled for the
    /// held item and its enchantments, the item's wear, the mined statistic,
    /// the exhaustion and the experience. True when the block was removed.
    ///
    /// With `into_inventory`, the block's own drops go to the player's
    /// inventory instead of popping, and what doesn't fit is dropped from
    /// them as `Inventory.placeItemBackInInventory` drops it; what else the
    /// break sets loose (a torch that loses its support, a container's
    /// contents) still drops in the world. A creative player gets no drops.
    ///
    /// The player is the acting one inside their own hook (the usual place:
    /// their `native_destroy_block`, as NeoForge's `BlockEvent.BreakEvent`),
    /// or any online player from a hook with no player swapped in, such as
    /// `native_server_tick`. False, with nothing broken, for anyone else
    /// while a player acts, for a player who isn't here or is dead, for air,
    /// an unloaded block, a block no survival player can break (bedrock),
    /// and off a server. The player needn't reach or look at the block.
    /// Breaking from `native_destroy_block` makes it hear these breaks too:
    /// a mod keeps its own guard against breaking again from them.
    /// API @API:player_break@.
    pub(super) fn native_player_break_block(
        &mut self,
        uuid: u128,
        pos: [i32; 3],
        into_inventory: bool,
    ) -> bool {
        if !self.native_authoritative() {
            return false;
        }
        let mut broke = false;
        self.native_as_player(uuid, |game| {
            broke = game.break_as_acting_player(pos, into_inventory);
        });
        broke
    }

    fn break_as_acting_player(&mut self, pos: [i32; 3], into_inventory: bool) -> bool {
        let kept = into_inventory.then(BreakKept::default);
        let Some((broke, kept)) = self.break_keeping(pos, kept) else {
            return false;
        };
        for stack in kept.map(|k| k.stacks).unwrap_or_default() {
            self.give_or_drop(stack);
        }
        broke
    }

    /// `native_player_break_block`, with the block's own drops and its
    /// experience kept for the mod instead of popping: the break is the same
    /// player's break (their checks, `native_destroy_block`, the loot for
    /// their tool, its wear, the statistic, the exhaustion), but nothing of
    /// the block itself lands in the world, so a vein miner can gather a
    /// whole vein's drops and set them down in one place
    /// (`native_pop_break_drops`), as FTB Ultimine does. What else the break
    /// sets loose (a torch losing its support, a container's contents) still
    /// drops in the world. None, with nothing broken, wherever
    /// `native_player_break_block` answers false; a creative player's break
    /// keeps nothing. With the `doTileDrops` rule off nothing is kept.
    /// API @API:player_break_drops@.
    pub(super) fn native_player_break_block_kept(
        &mut self,
        uuid: u128,
        pos: [i32; 3],
    ) -> Option<NativeBreakDrops> {
        if !self.native_authoritative() {
            return None;
        }
        let mut answer = None;
        self.native_as_player(uuid, |game| {
            let kept = BreakKept {
                stacks: Vec::new(),
                experience: Some(0),
            };
            if let Some((true, Some(kept))) = game.break_keeping(pos, Some(kept)) {
                answer = Some(NativeBreakDrops {
                    stacks: kept.stacks,
                    experience: kept.experience.unwrap_or(0),
                });
            }
        });
        answer
    }

    /// Sets down drops a mod gathered (`native_player_break_block_kept`) at
    /// `pos` in the level of the player `uuid`: equal stacks merge into as
    /// few as their size allows (FTB Ultimine's `ItemCollection`), each pops
    /// there as `Block.popResource` pops a block's drop, and `experience`
    /// comes out at the block's centre as orbs (`ExperienceOrb.award`).
    /// False, with nothing set down, for a player who isn't here or is dead,
    /// another player than the one acting, an unloaded `pos`, and off a
    /// server. API @API:player_break_drops@.
    pub(super) fn native_pop_break_drops(
        &mut self,
        uuid: u128,
        pos: [i32; 3],
        stacks: Vec<ItemStack>,
        experience: u32,
    ) -> bool {
        if !self.native_authoritative() {
            return false;
        }
        let mut done = false;
        self.native_as_player(uuid, |game| {
            if !game.world.loaded(pos[0], pos[2]) {
                return;
            }
            for stack in merge_stacks(stacks, |s| game.max_stack(s)) {
                let at = super::spill::pop_position(&mut game.world.random, pos);
                game.spawn_item_own(stack, at, 10);
            }
            if experience > 0 {
                game.spawn_orbs(pos.map(|c| f64::from(c) + 0.5), experience);
            }
            done = true;
        });
        done
    }

    /// Breaks `pos` as the acting player with their drops going to `kept`
    /// (popped when None): whether it broke and what was kept, or None for
    /// a block the checks before the break refuse.
    fn break_keeping(
        &mut self,
        pos: [i32; 3],
        kept: Option<BreakKept>,
    ) -> Option<(bool, Option<BreakKept>)> {
        if !self.world.loaded(pos[0], pos[2]) {
            return None;
        }
        let state = self.world.block(pos[0], pos[1], pos[2]);
        // A survival player never finishes mining a block with no hardness:
        // their client never sends the break.
        if self.res.behaviours().is_air(state)
            || (!self.creative && self.res.facts().get(state).hardness < 0.0)
        {
            return None;
        }
        // ServerPlayerGameMode.handleBlockBreakAction's `mayInteract`.
        if self
            .lan_acting_spawn_protection
            .is_some_and(|radius| self.spawn_protects(radius, self.dim.id(), pos))
        {
            return None;
        }
        // A break inside another's hook keeps where the outer one's drops go.
        let outer = std::mem::replace(&mut self.break_collect, kept);
        let broke = self.player_break_block(pos).unwrap_or_else(|e| {
            log::warn!("a mod's break at {pos:?} failed: {e}");
            false
        });
        let kept = std::mem::replace(&mut self.break_collect, outer);
        Some((broke, kept))
    }

    /// `Block.popExperience` for a player's break: true when the break keeps
    /// its experience for a mod (`native_player_break_block_kept`) instead.
    pub(super) fn keep_break_experience(&mut self, xp: u32) -> bool {
        match self
            .break_collect
            .as_mut()
            .and_then(|k| k.experience.as_mut())
        {
            Some(kept) => {
                *kept = kept.saturating_add(xp);
                true
            }
            None => false,
        }
    }
}

/// Equal stacks (same item and components) merged into as few as `max`
/// allows, in the order each first appeared.
fn merge_stacks(stacks: Vec<ItemStack>, max: impl Fn(&ItemStack) -> u32) -> Vec<ItemStack> {
    let mut merged: Vec<ItemStack> = Vec::new();
    for mut stack in stacks {
        let limit = max(&stack).max(1);
        for into in merged.iter_mut() {
            if stack.count == 0 {
                break;
            }
            if into.count < limit && into.same_item_same_components(&stack) {
                let moved = (limit - into.count).min(stack.count);
                into.count += moved;
                stack.count -= moved;
            }
        }
        while stack.count > 0 {
            let count = stack.count.min(limit);
            stack.count -= count;
            merged.push(ItemStack {
                count,
                ..stack.clone()
            });
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::super::native_machines::tests::server_game;
    use super::*;
    use horizon_client::{command::args::GameMode, items::ItemStack, motion::PlayerMotion};
    use horizon_core::geometry::Vec3;

    /// A server's game with its own player in survival standing at the
    /// origin chunk, holding `held`: their UUID.
    fn with_player(game: &mut Game<'_>, held: ItemStack) -> u128 {
        game.player = Some(PlayerMotion::new(game.motion, Vec3::new(4.5, 64.0, 4.5)).unwrap());
        game.set_mode(GameMode::Survival);
        game.inventory[game.selected] = Some(held);
        game.native_acting_id()
    }
    fn place(game: &mut Game<'_>, pos: [i32; 3], block: &str) {
        let state = game.pack.assets.registry.default_state(block).unwrap();
        game.world.set(pos[0], pos[1], pos[2], state).unwrap();
    }
    fn block(game: &Game<'_>, pos: [i32; 3]) -> String {
        let state = game.world.block(pos[0], pos[1], pos[2]);
        game.res.facts().get(state).name.to_string()
    }
    fn carried(game: &Game<'_>, item: &str) -> u32 {
        let held = game.inventory.iter().flatten();
        held.filter(|s| &*s.item == item).map(|s| s.count).sum()
    }
    fn popped(game: &Game<'_>, item: &str) -> bool {
        game.dropped.iter().any(|e| &*e.stack.item == item)
    }

    /// Stone broken with a diamond pickaxe drops cobblestone, wears the
    /// pickaxe by one and counts as mined (`Block.playerDestroy`,
    /// `ItemStack.mineBlock`), as a player's own break does.
    #[test]
    #[ignore = "needs prepared resources in HORIZON_TEST_RESOURCES"]
    fn a_mod_breaks_stone_as_the_player_with_their_pickaxe() {
        let mut game = server_game();
        let uuid = with_player(&mut game, ItemStack::new("minecraft:diamond_pickaxe", 1));
        let pos = [6, 64, 4];
        place(&mut game, pos, "minecraft:stone");
        assert!(game.native_player_break_block(uuid, pos, false));
        assert_eq!(block(&game, pos), "minecraft:air");
        assert!(popped(&game, "minecraft:cobblestone"));
        assert_eq!(game.held().unwrap().damage, 1);
        assert_eq!(game.stats.get("mined", "minecraft:stone"), 1);
    }

    /// Into the inventory: silk touch keeps the stone whole, and a diamond
    /// ore gives its diamond and still its experience (3 to 7 points).
    #[test]
    #[ignore = "needs prepared resources in HORIZON_TEST_RESOURCES"]
    fn drops_go_into_the_inventory_rolled_for_the_tools_enchantments() {
        let mut game = server_game();
        let mut silk = ItemStack::new("minecraft:diamond_pickaxe", 1);
        silk.enchant("minecraft:silk_touch", 1);
        let uuid = with_player(&mut game, silk);
        let (stone, ore) = ([6, 64, 4], [7, 64, 4]);
        place(&mut game, stone, "minecraft:stone");
        assert!(game.native_player_break_block(uuid, stone, true));
        assert_eq!(carried(&game, "minecraft:stone"), 1);
        assert!(!popped(&game, "minecraft:stone"));

        game.inventory[game.selected] = Some(ItemStack::new("minecraft:diamond_pickaxe", 1));
        place(&mut game, ore, "minecraft:diamond_ore");
        assert!(game.native_player_break_block(uuid, ore, true));
        assert_eq!(carried(&game, "minecraft:diamond"), 1);
        assert!(!game.orbs.is_empty(), "the ore's experience");
    }

    /// Kept: the ore's diamond and its experience come back instead of
    /// landing, the stone is gone, and popping them elsewhere sets them
    /// down there with the experience as orbs.
    #[test]
    #[ignore = "needs prepared resources in HORIZON_TEST_RESOURCES"]
    fn kept_drops_come_back_and_pop_where_the_mod_says() {
        let mut game = server_game();
        let uuid = with_player(&mut game, ItemStack::new("minecraft:diamond_pickaxe", 1));
        let (ore, origin) = ([7, 64, 4], [6, 64, 6]);
        place(&mut game, ore, "minecraft:diamond_ore");
        let kept = game.native_player_break_block_kept(uuid, ore).unwrap();
        assert_eq!(block(&game, ore), "minecraft:air");
        assert_eq!(kept.stacks.len(), 1);
        assert_eq!(&*kept.stacks[0].item, "minecraft:diamond");
        assert!((3..=7).contains(&kept.experience));
        assert!(!popped(&game, "minecraft:diamond"));
        assert!(game.orbs.is_empty());
        assert_eq!(game.held().unwrap().damage, 1, "still the player's break");
        assert!(
            game.native_player_break_block_kept(uuid, ore).is_none(),
            "air"
        );

        assert!(game.native_pop_break_drops(uuid, origin, kept.stacks, kept.experience));
        let diamond = game
            .dropped
            .iter()
            .find(|e| &*e.stack.item == "minecraft:diamond");
        let at = diamond.unwrap().position;
        assert!((at.x - 6.5).abs() <= 0.25 && (at.z - 6.5).abs() <= 0.25);
        assert!(!game.orbs.is_empty());
    }

    /// Equal stacks fill up to their size, in first-seen order; different
    /// components stay apart.
    #[test]
    fn gathered_drops_merge_into_as_few_stacks_as_fit() {
        let stack = |item: &str, count| ItemStack::new(item, count);
        let mut named = stack("minecraft:cobblestone", 5);
        named.enchant("minecraft:unbreaking", 1);
        let merged = merge_stacks(
            vec![
                stack("minecraft:cobblestone", 40),
                stack("minecraft:raw_iron", 2),
                named.clone(),
                stack("minecraft:cobblestone", 40),
                stack("minecraft:ender_pearl", 10),
                stack("minecraft:raw_iron", 1),
            ],
            |s| {
                if &*s.item == "minecraft:ender_pearl" {
                    16
                } else {
                    64
                }
            },
        );
        let counts: Vec<(&str, u32)> = merged.iter().map(|s| (&*s.item, s.count)).collect();
        assert_eq!(
            counts,
            [
                ("minecraft:cobblestone", 64),
                ("minecraft:raw_iron", 3),
                ("minecraft:cobblestone", 5),
                ("minecraft:cobblestone", 16),
                ("minecraft:ender_pearl", 10),
            ]
        );
        assert_eq!(merged[2], named);
    }

    /// The player's own checks hold: an adventure player whose tool has no
    /// `can_break`, a survival player at bedrock, air, and someone who isn't
    /// playing break nothing.
    #[test]
    #[ignore = "needs prepared resources in HORIZON_TEST_RESOURCES"]
    fn the_players_checks_hold() {
        let mut game = server_game();
        let uuid = with_player(&mut game, ItemStack::new("minecraft:diamond_pickaxe", 1));
        let (stone, bedrock, air) = ([6, 64, 4], [7, 64, 4], [8, 64, 4]);
        place(&mut game, stone, "minecraft:stone");
        place(&mut game, bedrock, "minecraft:bedrock");
        assert!(!game.native_player_break_block(uuid, bedrock, false));
        assert_eq!(block(&game, bedrock), "minecraft:bedrock");
        assert!(!game.native_player_break_block(uuid, air, false));
        assert!(!game.native_player_break_block(uuid ^ 1, stone, false));
        game.set_mode(GameMode::Adventure);
        assert!(!game.native_player_break_block(uuid, stone, false));
        assert_eq!(block(&game, stone), "minecraft:stone");
        assert_eq!(game.held().unwrap().damage, 0, "nothing wore the pickaxe");
    }
}
