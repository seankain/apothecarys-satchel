# 03 — Combat & Party System

## Scope

Turn-based combat system, autonomous party member AI, DnD-style stats, party generation with permadeath, recruitment, and the player's unique apothecary role in combat.

## Character Stats (DnD-Analogous)

### Core Attributes

```rust
// crates/core/src/stats.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attributes {
    pub strength: i32,      // Melee damage, carry capacity
    pub dexterity: i32,     // Initiative, dodge, ranged accuracy
    pub constitution: i32,  // HP pool, poison/disease resistance
    pub intelligence: i32,  // Magic damage, crafting insight (apothecary only)
    pub wisdom: i32,        // Perception, status resist, AI decision quality
    pub charisma: i32,      // Recruitment cost, NPC prices, party morale
}

impl Attributes {
    /// Standard DnD modifier: (stat - 10) / 2
    pub fn modifier(stat: i32) -> i32 {
        (stat - 10) / 2
    }
}
```

### Derived Stats

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivedStats {
    pub max_hp: i32,            // 10 + CON_mod * level + class_bonus
    pub current_hp: i32,
    pub armor_class: i32,       // 10 + DEX_mod + equipment
    pub initiative_bonus: i32,  // DEX_mod + class_bonus
    pub movement_speed: f32,    // Base 5.0 + DEX_mod * 0.5
    pub attack_bonus: i32,      // STR_mod or DEX_mod + proficiency
    pub damage_dice: DamageDice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DamageDice {
    pub count: u32,     // Number of dice
    pub sides: u32,     // d4, d6, d8, d10, d12, d20
    pub bonus: i32,     // Flat modifier
}
```

### Status Effects

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatusEffect {
    // Buffs (from potions/medicines)
    AttackUp { amount: i32, turns: u32 },
    DefenseUp { amount: i32, turns: u32 },
    Regeneration { hp_per_turn: i32, turns: u32 },
    Haste { extra_action: bool, turns: u32 },
    Resistance { damage_type: DamageType, turns: u32 },

    // Debuffs (from enemies, bad potions)
    Poisoned { damage_per_turn: i32, turns: u32 },
    Weakened { attack_penalty: i32, turns: u32 },
    Slowed { turns: u32 },
    Stunned { turns: u32 },
    Blinded { turns: u32 },

    // Special (from rare plants)
    StatBoost { attribute: AttributeType, amount: i32, turns: u32 },
}
```

**Task Goal**: Implement the full stat system in `crates/core/src/stats.rs` with attribute modifiers, derived stat calculation, and status effect application/tick/expiry.

## Party Member Classes

Party members (not the player) belong to combat classes:

| Class | Role | Primary Stat | Abilities |
|-------|------|-------------|-----------|
| Warrior | Melee tank | STR/CON | Heavy attacks, taunt, shield block |
| Ranger | Ranged DPS | DEX | Bow attacks, multi-shot, evasion |
| Mage | Magic DPS/CC | INT | Elemental spells, AoE, debuffs |
| Cleric | Support | WIS | Minor heals, bless, turn undead |
| Rogue | Burst DPS | DEX | Sneak attack, poison blade, dodge |

The player character is always **Apothecary** — cannot attack, can only use/give items and craft.

## Party Generation

### Procedural Generation

Party members are procedurally generated with:
1. Random name (from name tables)
2. Random class
3. Attributes rolled: `3d6` for each, with class-based minimum thresholds
4. Random personality traits (affect AI behavior in combat)
5. Visual appearance (random selection from archetype mesh variants + color palettes)
6. Background flavor text (assembled from templates)

```rust
// crates/party/src/generation.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartyMember {
    pub id: Uuid,
    pub name: String,
    pub class: PartyClass,
    pub level: u32,
    pub xp: u32,
    pub attributes: Attributes,
    pub derived: DerivedStats,
    pub personality: Personality,
    pub status_effects: Vec<ActiveStatusEffect>,
    pub equipment: Equipment,
    pub appearance: AppearanceData,
    pub alive: bool,   // false = permadead
    pub backstory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Personality {
    pub aggression: f32,    // 0.0 = defensive, 1.0 = all-out attack
    pub caution: f32,       // 0.0 = reckless, 1.0 = self-preserving
    pub team_focus: f32,    // 0.0 = selfish, 1.0 = protects allies
    pub item_affinity: f32, // 0.0 = never uses items, 1.0 = prefers items
}

pub fn generate_party_member(rng: &mut impl Rng, level: u32) -> PartyMember;
```

**Task Goal**: Implement party member generation with stat rolling, personality randomization, and appearance selection. Generate a pool of 3–6 recruitable members for the hub.

### Permadeath

When a party member's HP reaches 0 in combat:
1. They are marked `alive: false`
2. Death animation plays
3. They are permanently removed from the party roster
4. Their equipment drops to party inventory
5. A new recruit is eventually generated in the hub to replace them

```rust
// crates/party/src/permadeath.rs
pub fn handle_death(member: &mut PartyMember, party_inventory: &mut Inventory) {
    member.alive = false;
    // Transfer equipment to party inventory
    party_inventory.add_all(member.equipment.unequip_all());
    // Queue notification for UI
}
```

## Recruitment (Hub)

At the hub, the player can:
1. View available recruits (pool of 3–6 generated members)
2. Inspect their stats, personality, and backstory
3. Recruit them (may cost gold or items depending on charisma)
4. Dismiss party members (they leave permanently)

The recruitment pool refreshes after each dungeon run (old unrecruited members leave, new ones arrive).

**Task Goal**: Implement recruitment UI and roster management in `crates/party/src/recruitment.rs` and `crates/party/src/roster.rs`.

## Combat System

### Combat Initiation

Combat triggers when:
1. Player encounters an enemy group in a dungeon
2. Scripted encounter (Lua trigger)
3. Boss encounter (entering specific area)

```
Encounter triggered
         │
         ▼
Snapshot dungeon state
         │
         ▼
Transition to combat scene/mode
         │
         ▼
Place party + enemies on combat grid
         │
         ▼
Roll initiative for all combatants
         │
         ▼
Begin turn cycle
```

### Turn Structure

```rust
// crates/combat/src/turn_manager.rs

pub struct CombatState {
    pub combatants: Vec<Combatant>,     // Sorted by initiative
    pub current_turn: usize,            // Index into combatants
    pub round: u32,
    pub phase: CombatPhase,
}

pub enum CombatPhase {
    RollInitiative,
    TurnStart { combatant_id: Uuid },
    ActionSelection,       // AI selects for party members; player selects items
    ActionExecution,       // Animate and resolve
    TurnEnd,               // Tick status effects, check deaths
    RoundEnd,              // Check victory/defeat
    Victory { xp: u32, loot: Vec<ItemDrop> },
    Defeat,
}
```

### Turn Flow

```
┌──────────────────────────────────────────────────┐
│                  ROUND START                      │
│  Sort combatants by initiative (DEX_mod + d20)   │
└──────────────┬───────────────────────────────────┘
               │
               ▼
┌──────────────────────────────────────────────────┐
│              TURN: Combatant[i]                   │
│                                                   │
│  Is player character?                             │
│    YES → Show item/potion menu                    │
│           Player selects item + target            │
│    NO (party member) → AI selects action          │
│    NO (enemy) → Enemy AI selects action           │
│                                                   │
│  Execute action:                                  │
│    1. Play animation                              │
│    2. Roll to hit (if applicable)                 │
│    3. Calculate damage/effect                     │
│    4. Apply to target(s)                          │
│    5. Check for death                             │
│                                                   │
│  End of turn:                                     │
│    - Tick status effects (reduce duration)        │
│    - Remove expired effects                       │
│    - Apply DoT damage                             │
└──────────────┬───────────────────────────────────┘
               │
               ▼
        Next combatant (or round end)
               │
               ▼
        All enemies dead? → VICTORY
        All party dead? → DEFEAT
        Otherwise → next round
```

**Task Goal**: Implement `CombatState` and turn cycling in `crates/combat/src/turn_manager.rs`. Must handle initiative rolling, turn ordering, phase transitions, and victory/defeat detection.

### Player Actions (Apothecary)

The player **cannot attack**. Available actions:

| Action | Effect |
|--------|--------|
| Use Potion on Ally | Apply potion effect (heal, buff, cure) |
| Use Potion on Enemy | Throw harmful potion (poison, debuff) |
| Give Item to Ally | Transfer consumable for ally's own use |
| Examine Enemy | Reveal enemy stats/weaknesses (INT check) |
| Wait | Skip turn |

```rust
// crates/combat/src/actions.rs

pub enum PlayerAction {
    UseItem { item_id: Uuid, target: CombatantId },
    GiveItem { item_id: Uuid, target: CombatantId },
    Examine { target: CombatantId },
    Wait,
}
```

**Task Goal**: Implement player action resolution. Item effects are defined in the item data; the combat system applies them via the status effect system.

### Party Member AI

Each party member selects actions autonomously based on their personality, current state, and battlefield conditions.

```rust
// crates/combat/src/ai.rs

pub fn select_action(
    member: &PartyMember,
    allies: &[&Combatant],
    enemies: &[&Combatant],
    available_items: &[&Item],
) -> CombatAction {
    // 1. Evaluate urgency scores
    let self_danger = evaluate_self_danger(member);       // HP%, status effects
    let ally_danger = evaluate_ally_danger(allies);        // Any ally critical?
    let best_target = evaluate_targets(enemies, member);  // Weakest, most threatening

    // 2. Decision tree influenced by personality
    if self_danger > 0.8 && member.personality.caution > 0.5 {
        return defensive_action(member);  // Heal self, dodge, retreat
    }

    if ally_danger > 0.7 && member.personality.team_focus > 0.6 {
        return support_action(member, allies);  // Protect, heal, taunt
    }

    if member.personality.aggression > 0.7 {
        return aggressive_action(member, best_target);  // Strongest attack
    }

    // 3. Default: balanced action
    balanced_action(member, allies, enemies)
}
```

**Decision Factors**:
- **Self HP%**: Low HP → defensive actions (if cautious personality)
- **Ally HP%**: Low ally HP → support actions (if team-focused)
- **Enemy count**: Many enemies → AoE preference (if available)
- **Status effects**: Affected by debuffs → try to use items to cure (if high item_affinity)
- **Class abilities**: Each class has a prioritized action list

**Task Goal**: Implement the AI decision system in `crates/combat/src/ai.rs`. The AI should produce reasonable tactical behavior without being optimal (personality introduces intentional suboptimality for character flavor).

### Enemy AI

Enemies use a simpler priority system defined per enemy type in data:

```ron
// data/enemies.ron
EnemyTemplate(
    id: "goblin_warrior",
    base_stats: Attributes(strength: 12, dexterity: 14, ...),
    actions: [
        EnemyAction(name: "Slash", weight: 3, damage: "1d6+2", target: LowestHp),
        EnemyAction(name: "Shield Bash", weight: 1, damage: "1d4", effect: Stunned(1), target: HighestThreat),
    ],
)
```

## Combat Rewards

On victory:
1. XP distributed to surviving party members
2. Loot table rolled per enemy
3. Plant samples may drop (with hidden genetics)
4. Gold/currency awarded
5. Return to dungeon exploration state

## Key Implementation Files

| File | Purpose |
|------|---------|
| `crates/core/src/stats.rs` | Attributes, derived stats, modifiers |
| `crates/core/src/components.rs` | Combatant, StatusEffect components |
| `crates/combat/src/turn_manager.rs` | Turn cycling, phase management |
| `crates/combat/src/actions.rs` | Action definitions and resolution |
| `crates/combat/src/ai.rs` | Party member autonomous AI |
| `crates/combat/src/status.rs` | Status effect application and ticking |
| `crates/party/src/generation.rs` | Procedural party member creation |
| `crates/party/src/permadeath.rs` | Death handling |
| `crates/party/src/recruitment.rs` | Hub recruitment mechanics |
| `crates/party/src/roster.rs` | Active party management |
| `data/enemies.ron` | Enemy templates |
| `data/party_templates.ron` | Generation tables |
