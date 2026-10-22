// SPDX-License-Identifier: GPL-3.0-or-later
//! Enumeration values of `Turbine.Gameplay` (generated from `lua-plugin-api.md` Appendix
//! A.1 and A.4; interoperability values).

/// `Turbine.Gameplay.Alignment` (`lua-plugin-api.md` Appendix A).
pub(crate) const ALIGNMENT: &[(&str, u32)] =
    &[("Undefined", 0), ("FreePeople", 1), ("MonsterPlayer", 2)];

/// `Turbine.Gameplay.Class` (`lua-plugin-api.md` Appendix A).
pub(crate) const CLASS: &[(&str, u32)] = &[
    ("Undefined", 0),
    ("Guardian", 23),
    ("Captain", 24),
    ("Minstrel", 31),
    ("Burglar", 40),
    ("WarLeader", 52),
    ("Reaver", 71),
    ("Stalker", 126),
    ("Weaver", 127),
    ("Defiler", 128),
    ("Hunter", 162),
    ("Champion", 172),
    ("BlackArrow", 179),
    ("LoreMaster", 185),
    ("Troll", 190),
    ("Ranger", 191),
    ("Chicken", 192),
    ("RuneKeeper", 193),
    ("Warden", 194),
    ("Beorning", 214),
    ("Brawler", 215),
    ("Mariner", 216),
];

/// `Turbine.Gameplay.Race` (`lua-plugin-api.md` Appendix A).
pub(crate) const RACE: &[(&str, u32)] = &[
    ("Undefined", 0),
    ("Man", 23),
    ("Elf", 65),
    ("Dwarf", 73),
    ("Hobbit", 81),
    ("Beorning", 114),
    ("HighElf", 117),
    ("StoutAxe", 120),
    ("RiverHobbit", 125),
];

/// `Turbine.Gameplay.Equipment` (`lua-plugin-api.md` Appendix A).
pub(crate) const EQUIPMENT: &[(&str, u32)] = &[
    ("Undefined", 0),
    ("Head", 1),
    ("Chest", 2),
    ("Legs", 3),
    ("Gloves", 4),
    ("Boots", 5),
    ("Shoulder", 6),
    ("Back", 7),
    ("Bracelet1", 8),
    ("Bracelet2", 9),
    ("Necklace", 10),
    ("Ring1", 11),
    ("Ring2", 12),
    ("Earring1", 13),
    ("Earring2", 14),
    ("Pocket", 15),
    ("PrimaryWeapon", 16),
    ("SecondaryWeapon", 17),
    ("Shield", 17),
    ("Instrument", 18),
    ("RangedWeapon", 18),
    ("CraftTool", 19),
    ("Class", 20),
    ("Bridle", 21),
];

/// `Turbine.Gameplay.EffectCategory` (`lua-plugin-api.md` Appendix A).
pub(crate) const EFFECTCATEGORY: &[(&str, u32)] = &[
    ("Undefined", 0),
    ("Disease", 2),
    ("Physical", 4),
    ("Wound", 8),
    ("Cry", 16),
    ("Song", 32),
    ("Fear", 64),
    ("Poison", 128),
    ("Elemental", 512),
    ("Corruption", 1024),
    ("Dispellable", 1226),
];

/// `Turbine.Gameplay.ItemQuality` (`lua-plugin-api.md` Appendix A).
pub(crate) const ITEMQUALITY: &[(&str, u32)] = &[
    ("Undefined", 0),
    ("Legendary", 1),
    ("Rare", 2),
    ("Incomparable", 3),
    ("Uncommon", 4),
    ("Common", 5),
];

/// `Turbine.Gameplay.ItemDurability` (`lua-plugin-api.md` Appendix A).
pub(crate) const ITEMDURABILITY: &[(&str, u32)] = &[
    ("Undefined", 0),
    ("Substantial", 1),
    ("Brittle", 2),
    ("Normal", 3),
    ("Tough", 4),
    ("Flimsy", 5),
    ("Indestructible", 6),
    ("Weak", 7),
];

/// `Turbine.Gameplay.ItemWearState` (`lua-plugin-api.md` Appendix A).
pub(crate) const ITEMWEARSTATE: &[(&str, u32)] = &[
    ("Undefined", 0),
    ("Damaged", 1),
    ("Pristine", 2),
    ("Broken", 3),
    ("Worn", 4),
];

/// `Turbine.Gameplay.ReadyState` (`lua-plugin-api.md` Appendix A).
pub(crate) const READYSTATE: &[(&str, u32)] = &[("Unset", 0), ("Ready", 1), ("NotReady", 2)];

/// `Turbine.Gameplay.SkillType` (`lua-plugin-api.md` Appendix A).
pub(crate) const SKILLTYPE: &[(&str, u32)] = &[("Normal", 0), ("Mount", 1), ("Gambit", 2)];

/// `Turbine.Gameplay.Profession` (`lua-plugin-api.md` Appendix A).
pub(crate) const PROFESSION: &[(&str, u32)] = &[
    ("Undefined", 0),
    ("Metalsmith", 1),
    ("Forester", 2),
    ("Scholar", 3),
    ("Jeweller", 4),
    ("Tailor", 5),
    ("Weaponsmith", 6),
    ("Prospector", 7),
    ("Woodworker", 8),
    ("Farmer", 9),
    ("Cook", 10),
];

/// `Turbine.Gameplay.Vocation` (`lua-plugin-api.md` Appendix A).
pub(crate) const VOCATION: &[(&str, u32)] = &[
    ("None", 0),
    ("Explorer", 1),
    ("Tinker", 2),
    ("Yeoman", 3),
    ("Historian", 4),
    ("Armsman", 5),
    ("Woodsman", 6),
    ("Armorer", 7),
];

/// `Turbine.Gameplay.CraftTier` (`lua-plugin-api.md` Appendix A).
pub(crate) const CRAFTTIER: &[(&str, u32)] = &[
    ("Undefined", 0),
    ("Apprentice", 1),
    ("Journeyman", 2),
    ("Expert", 3),
    ("Artisan", 4),
    ("Master", 5),
    ("Supreme", 6),
    ("Westfold", 7),
    ("Eastemnet", 8),
    ("Westemnet", 8),
];

/// `Turbine.Gameplay.Attributes.HunterStance` (`lua-plugin-api.md` Appendix A).
pub(crate) const HUNTERSTANCE: &[(&str, u32)] = &[
    ("None", 0),
    ("Precision", 1),
    ("Strength", 2),
    ("Endurance", 3),
];

/// `Turbine.Gameplay.Attributes.MinstrelStance` (`lua-plugin-api.md` Appendix A).
pub(crate) const MINSTRELSTANCE: &[(&str, u32)] = &[("None", 0), ("WarSpeech", 1), ("Harmony", 2)];

/// `Turbine.Gameplay.Attributes.WardenStance` (`lua-plugin-api.md` Appendix A).
pub(crate) const WARDENSTANCE: &[(&str, u32)] =
    &[("None", 0), ("Determination", 2), ("Assailment", 3)];

/// `Turbine.Gameplay.Attributes.WardenGambit` (`lua-plugin-api.md` Appendix A).
pub(crate) const WARDENGAMBIT: &[(&str, u32)] =
    &[("Spear", 1), ("Shield", 2), ("Fist", 3), ("Javelin", 4)];

/// `Turbine.Gameplay.Attributes.BurglarStance` (`lua-plugin-api.md` Appendix A).
pub(crate) const BURGLARSTANCE: &[(&str, u32)] = &[("None", 0)];

/// `Turbine.Gameplay.Attributes.ChampionStance` (`lua-plugin-api.md` Appendix A).
pub(crate) const CHAMPIONSTANCE: &[(&str, u32)] = &[("None", 0)];

/// `Turbine.Gameplay.Attributes.GuardianStance` (`lua-plugin-api.md` Appendix A).
pub(crate) const GUARDIANSTANCE: &[(&str, u32)] = &[("None", 0)];

/// `Turbine.Gameplay.Attributes.BrawlerStance` (`lua-plugin-api.md` Appendix A).
pub(crate) const BRAWLERSTANCE: &[(&str, u32)] = &[("None", 0)];

/// `Turbine.Gameplay.ItemCategory` (`lua-plugin-api.md` Appendix A).
pub(crate) const ITEMCATEGORY: &[(&str, u32)] = &[
    ("Undefined", 0),
    ("Bow", 1),
    ("Treasure", 2),
    ("Chest", 3),
    ("Minstrel", 4),
    ("Hands", 5),
    ("Shoulders", 6),
    ("Head", 7),
    ("Mounts", 8),
    ("Pennant", 9),
    ("Dagger", 10),
    ("Instrument", 11),
    ("Axe", 12),
    ("Captain", 13),
    ("Oil", 14),
    ("Legs", 15),
    ("Effect", 16),
    ("Hunter", 17),
    ("Armor", 18),
    ("Loremaster", 19),
    ("Trap", 20),
    ("Decoration", 21),
    ("Champion", 22),
    ("Feet", 23),
    ("Hammer", 24),
    ("Healing", 25),
    ("Guardian", 26),
    ("Quest", 27),
    ("Potion", 28),
    ("Crossbow", 29),
    ("Mace", 30),
    ("Key", 31),
    ("Tool", 32),
    ("Shield", 33),
    ("Staff", 34),
    ("Crafting", 35),
    ("Halberd", 36),
    ("Component", 37),
    ("Ingredient", 38),
    ("Clothing", 39),
    ("Club", 40),
    ("Dye", 41),
    ("Weapon", 42),
    ("KinshipCharter", 43),
    ("Sword", 44),
    ("Back", 45),
    ("Spear", 46),
    ("Trophy", 47),
    ("Burglar", 48),
    ("Jewelry", 49),
    ("Device", 50),
    ("Relic", 51),
    ("Book", 52),
    ("NonInventory", 53),
    ("Thrown", 54),
    ("Food", 55),
    ("Resource", 56),
    ("Scroll", 57),
    ("ApprenticeTailorScroll", 58),
    ("ApprenticeJewellerScroll", 59),
    ("ApprenticeProspectingScroll", 60),
    ("ApprenticeFarmerScroll", 61),
    ("ApprenticeScholarScroll", 62),
    ("ApprenticeForestryScroll", 63),
    ("ApprenticeMetalworkScroll", 64),
    ("ApprenticeCookScroll", 65),
    ("ApprenticeWeaponsmithScroll", 66),
    ("ApprenticeWoodworkScroll", 67),
    ("MusicDecoration", 82),
    ("SurfacePaintDecoration", 83),
    ("TrophyDecoration", 84),
    ("FurnitureDecoration", 85),
    ("FloorDecoration", 86),
    ("YardDecoration", 87),
    ("WallDecoration", 88),
    ("Reputation", 89),
    ("Implement", 91),
    ("CosmeticHeld", 96),
    ("CosmeticBack", 97),
    ("SpecialDecoration", 98),
    ("CeilingDecoration", 99),
    ("FishingBait", 100),
    ("FishingOther", 101),
    ("Fish", 102),
    ("FishingPole", 103),
    ("Orb", 104),
    ("Warden", 105),
    ("Runekeeper", 106),
    ("LegendaryWeaponExperience", 107),
    ("LegendaryWeaponReset", 108),
    ("Deconstructable", 109),
    ("Javelin", 110),
    ("GuardianBelt", 111),
    ("JourneymanCookScroll", 113),
    ("ExpertCookScroll", 114),
    ("ArtisanCookScroll", 115),
    ("MasterCookScroll", 116),
    ("SupremeCookScroll", 117),
    ("MasterWoodworkScroll", 118),
    ("MasterWeaponsmithScroll", 119),
    ("ArtisanTailorScroll", 120),
    ("ExpertFarmerScroll", 121),
    ("JourneymanForestryScroll", 122),
    ("SupremeWoodworkScroll", 123),
    ("SupremeWeaponsmithScroll", 124),
    ("MasterTailorScroll", 125),
    ("ArtisanFarmerScroll", 126),
    ("ExpertForestryScroll", 127),
    ("SupremeTailorScroll", 128),
    ("MasterFarmerScroll", 129),
    ("ArtisanForestryScroll", 130),
    ("SupremeFarmerScroll", 131),
    ("MasterForestryScroll", 132),
    ("SupremeForestryScroll", 133),
    ("JourneymanProspectingScroll", 134),
    ("ExpertProspectingScroll", 135),
    ("ArtisanProspectingScroll", 136),
    ("MasterProspectingScroll", 137),
    ("SupremeProspectingScroll", 138),
    ("JourneymanScholarScroll", 139),
    ("ExpertScholarScroll", 140),
    ("JourneymanMetalworkScroll", 141),
    ("ArtisanScholarScroll", 142),
    ("JourneymanJewellerScroll", 143),
    ("ExpertMetalworkScroll", 144),
    ("MasterScholarScroll", 145),
    ("ExpertJewellerScroll", 146),
    ("JourneymanWoodworkScroll", 147),
    ("JourneymanWeaponsmithScroll", 148),
    ("ArtisanMetalworkScroll", 149),
    ("SupremeScholarScroll", 150),
    ("ArtisanJewellerScroll", 151),
    ("ExpertWoodworkScroll", 152),
    ("ExpertWeaponsmithScroll", 153),
    ("MasterMetalworkScroll", 154),
    ("JourneymanTailorScroll", 155),
    ("MasterJewellerScroll", 156),
    ("ArtisanWoodworkScroll", 157),
    ("ArtisanWeaponsmithScroll", 158),
    ("SupremeMetalworkScroll", 159),
    ("ExpertTailorScroll", 160),
    ("SupremeJewellerScroll", 161),
    ("JourneymanFarmerScroll", 162),
    ("MinstrelBook", 163),
    ("Misc", 164),
    ("LegendaryWeaponReplaceLegacy", 166),
    ("LegendaryWeaponIncreaseMaxLevel", 167),
    ("LegendaryWeaponUpgradeLegacy", 168),
    ("CraftingTrophy", 170),
    ("LoremasterFood", 171),
    ("SpecialTrophy", 172),
    ("Special", 173),
    ("Social", 174),
    ("Map", 175),
    ("LegendaryWeaponIncreaseItemLevel", 176),
    ("Skirmish", 177),
    ("Barter", 178),
    ("Shieldspike", 179),
    ("CosmeticFeet", 180),
    ("CosmeticShoulders", 181),
    ("CosmeticChest", 182),
    ("CosmeticHead", 183),
    ("CosmeticHands", 184),
    ("CosmeticLegs", 185),
    ("Horn", 187),
    ("OptionalIngredient", 188),
    ("Perk", 189),
    ("Tome", 190),
    ("Travel", 191),
    ("CosmeticClass", 192),
    ("LegendaryWeaponLegacyReveal", 193),
    ("LegendaryWeaponUnslotRelics", 194),
    ("WestfoldMetalworkScroll", 195),
    ("WestfoldJewellerScroll", 196),
    ("WestfoldWoodworkScroll", 197),
    ("WestfoldWeaponsmithScroll", 198),
    ("WestfoldTailorScroll", 199),
    ("WestfoldFarmerScroll", 200),
    ("WestfoldForestryScroll", 201),
    ("WestfoldProspectingScroll", 202),
    ("WestfoldCookScroll", 203),
    ("WestfoldScholarScroll", 204),
    ("Festival", 205),
    ("LegendaryWeaponAddLegacy", 206),
    ("Beorning", 233),
    ("Brawler", 288),
    ("Mariner", 294),
];
