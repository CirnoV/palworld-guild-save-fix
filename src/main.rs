pub mod character_save_parameter;
pub mod group_guild;
pub mod sav;

use std::{collections::HashSet, io::BufWriter, path::PathBuf};

use clap::Parser as ClapParser;
use indexmap::IndexMap;
use sav::read_save_file;
use uuid::Uuid;

use crate::{
    character_save_parameter::{write_raw_character_save_parameter, CharacterSaveParameter},
    group_guild::GroupGuildSave,
    sav::{
        get_character_save_parameter_map, get_character_save_parameter_map_mut,
        get_group_save_data_map, is_group_type_guild, parse_raw_group_guild_save, write_save_file,
        PalSave,
    },
};

#[derive(ClapParser, Debug)]
#[command(name = "palworld-guild-save-fix", about)]
struct Args {
    /// Input directory containing the save files (Level.sav and Players directory)
    input: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // 1. Read save files
    let sav_directory = std::path::Path::new(&args.input);
    let level_sav_path: PathBuf = sav_directory.join("Level.sav");
    let player_sav_paths: Vec<PathBuf> = std::fs::read_dir(sav_directory.join("Players"))
        .expect("Failed to read Players directory")
        .filter_map(|entry| entry.map(|entry| entry.path()).ok())
        .filter(|path| path.extension().map(|ext| ext == "sav").unwrap_or(false))
        .collect();
    let mut level_save = read_save_file(std::fs::File::open(&level_sav_path)?)?;
    println!("Level.sav read successfully");
    let player_saves: Vec<PalSave> = player_sav_paths
        .iter()
        .map(|path| read_save_file(std::fs::File::open(path)?))
        .collect::<Result<Vec<_>, _>>()?;
    println!("Player saves read successfully");

    // 2. Parse guild data from GroupSaveDataMap.RawData
    let groups: Vec<(Uuid, GroupGuildSave)> = get_group_save_data_map(&level_save)
        .iter()
        .filter(|entry| is_group_type_guild(entry))
        .map(|entry| {
            let uesave::PropertyValue::Struct(uesave::StructValue::Guid(guild_id)) = entry.key
            else {
                panic!()
            };
            (guild_id, parse_raw_group_guild_save(entry))
        })
        .collect();
    // Print guild infomation
    groups.iter().for_each(|(_, group)| {
        println!(
            "Guild {}({}) has {} members",
            group.GuildName,
            group.UnknownUuid,
            group.GuildPlayerInfo.len()
        );
        group.GuildPlayerInfo.iter().for_each(|player_info| {
            println!("- {}({})", player_info.PlayerName, player_info.PlayerUId);
        });
    });
    println!("Guilds parsed successfully");

    // 3. Parse character data from CharacterSaveParameterMap.RawData
    let character_save_parameter_map: HashSet<Uuid> = get_character_save_parameter_map(&level_save)
        .iter()
        .map(|entry| {
            let uesave::PropertyValue::Struct(uesave::StructValue::Struct(ref key)) = entry.key
            else {
                panic!()
            };
            let uesave::Property::Struct {
                value: uesave::StructValue::Guid(instance_id),
                ..
            } = &key["InstanceId"]
            else {
                panic!()
            };
            instance_id.clone()
        })
        .collect();
    println!("CharacterSaveParameterMap parsed successfully");

    // 4. Parse player individual ids from Player saves
    let player_individual_ids: Vec<(Uuid, Uuid)> = player_saves
        .iter()
        .map(|pal_save| {
            let uesave::Property::Struct {
                value: uesave::StructValue::Struct(save_data),
                ..
            } = &pal_save.save.root.properties["SaveData"]
            else {
                panic!()
            };
            let uesave::Property::Struct {
                value: uesave::StructValue::Struct(individual_id),
                ..
            } = &save_data["IndividualId"]
            else {
                panic!()
            };

            let player_uid = {
                let uesave::Property::Struct {
                    value: uesave::StructValue::Guid(player_uid),
                    ..
                } = &individual_id["PlayerUId"]
                else {
                    panic!()
                };
                player_uid.clone()
            };
            let instance_id = {
                let uesave::Property::Struct {
                    value: uesave::StructValue::Guid(instance_id),
                    ..
                } = &individual_id["InstanceId"]
                else {
                    panic!()
                };
                instance_id.clone()
            };

            (player_uid, instance_id)
        })
        .collect();
    player_individual_ids
        .iter()
        .for_each(|(player_uid, instance_id)| {
            println!("Player {} has individual id {}", player_uid, instance_id);
        });
    println!("Player individual ids parsed successfully");

    // 5. Check if player does not have a character save
    let player_without_character_save: Vec<(Uuid, Uuid)> = player_individual_ids
        .iter()
        .filter(|(_, instance_id)| !character_save_parameter_map.contains(instance_id))
        .map(|(player_uid, instance_id)| (player_uid.clone(), instance_id.clone()))
        .collect();
    player_without_character_save
        .iter()
        .for_each(|(player_uid, instance_id)| {
            println!(
                "Player {} has no character save with id {}",
                player_uid, instance_id
            );
        });
    // 5-1. When all players have a character save, exit
    if player_without_character_save.is_empty() {
        println!("All players have a character save. Exiting...");
        return Ok(());
    }

    // 6. Create a new character save for each player without a character save
    let template_character_save =
        include_str!("../templates/PalIndividualCharacterSaveParameter.json");
    let template_character_save: IndexMap<String, uesave::Property> =
        serde_json::from_str(template_character_save)?;
    let create_new_character_save = |nickname: &str, guild_id: &Uuid| -> CharacterSaveParameter {
        let mut character_save = template_character_save.clone();
        let uesave::Property::Struct {
            value: uesave::StructValue::Struct(properties),
            ..
        } = character_save.get_mut("SaveParameter").unwrap()
        else {
            panic!()
        };
        properties.insert(
            "NickName".into(),
            uesave::Property::Str {
                id: None,
                value: nickname.into(),
            },
        );
        CharacterSaveParameter {
            properties: character_save,
            group_id: guild_id.clone(),
        }
    };
    let new_character_saves: Vec<uesave::MapEntry> = player_without_character_save
        .iter()
        .map(|(player_uid, instance_id)| {
            let key = {
                let mut key: IndexMap<String, uesave::Property> = IndexMap::new();
                key.insert(
                    "PlayerUId".into(),
                    uesave::Property::Struct {
                        id: None,
                        value: uesave::StructValue::Guid(player_uid.clone()),
                        struct_type: uesave::StructType::Guid,
                        struct_id: Uuid::nil(),
                    },
                );
                key.insert(
                    "InstanceId".into(),
                    uesave::Property::Struct {
                        id: None,
                        value: uesave::StructValue::Guid(instance_id.clone()),
                        struct_type: uesave::StructType::Guid,
                        struct_id: Uuid::nil(),
                    },
                );
                key.insert(
                    "DebugName".into(),
                    uesave::Property::Str {
                        id: None,
                        value: "".into(),
                    },
                );
                uesave::PropertyValue::Struct(uesave::StructValue::Struct(key))
            };
            let value = {
                let character_save_parameter = groups
                    .iter()
                    .find_map(|(guild_id, group)| {
                        group
                            .GuildPlayerInfo
                            .iter()
                            .find(|player_info| player_info.PlayerUId == *player_uid)
                            .map(|player_info| {
                                create_new_character_save(&player_info.PlayerName, guild_id)
                            })
                    })
                    .unwrap();
                let mut value: IndexMap<String, uesave::Property> = IndexMap::new();
                value.insert(
                    "RawData".into(),
                    uesave::Property::Array {
                        array_type: uesave::PropertyType::ByteProperty,
                        id: None,
                        value: uesave::ValueArray::Base(uesave::ValueVec::Byte(
                            uesave::ByteArray::Byte(write_raw_character_save_parameter(
                                &level_save.save.header,
                            )(
                                &character_save_parameter
                            )),
                        )),
                    },
                );
                uesave::PropertyValue::Struct(uesave::StructValue::Struct(value))
            };
            uesave::MapEntry { key, value }
        })
        .collect::<Vec<_>>();
    println!("New character saves created successfully");

    // 7. Append new character saves to CharacterSaveParameterMap
    get_character_save_parameter_map_mut(&mut level_save).extend(new_character_saves);
    println!("New character saves appended successfully");

    // 8. Write Level.sav
    let level_sav_file = std::fs::File::create(level_sav_path)?;
    let mut level_sav_writer = BufWriter::new(level_sav_file);
    write_save_file(&mut level_sav_writer, &level_save)?;
    drop(level_sav_writer);
    println!("Level.sav written successfully");

    println!("All done! Press enter to exit...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    Ok(())
}
