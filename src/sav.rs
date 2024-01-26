use std::{
    io::{BufReader, Cursor, Read, Write},
    sync::Arc,
};

use byteorder::{LittleEndian, ReadBytesExt};
use indexmap::IndexMap;
use uesave::Save;
use winnow::Parser;

use crate::{
    character_save_parameter::{read_raw_character_save_parameter, CharacterSaveParameter},
    group_guild::{read_group_guild_save, stream, GroupGuildSave},
};

pub(crate) static SAVE_TYPES: once_cell::sync::Lazy<Arc<uesave::Types>> =
    once_cell::sync::Lazy::new(|| {
        let mut types = uesave::Types::new();
        types.add(
            ".worldSaveData.CharacterSaveParameterMap.Key".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.FoliageGridSaveDataMap.Key".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.FoliageGridSaveDataMap.ModelMap.InstanceDataMap.Key".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.MapObjectSpawnerInStageSaveData.Key".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.ItemContainerSaveData.Key".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.CharacterContainerSaveData.Key".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.CharacterContainerSaveData.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.GroupSaveDataMap.Key".into(),
            uesave::StructType::Guid,
        );
        types.add(
            ".worldSaveData.GroupSaveDataMap.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.WorkSaveData.WorkAssignMap.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
        ".worldSaveData.MapObjectSpawnerInStageSaveData.SpawnerDataMapByLevelObjectInstanceId.ItemMap.Value".into(),
        uesave::StructType::Struct(None),
    );
        types.add(
            ".worldSaveData.DungeonSaveData.MapObjectSaveData.Model.EffectMap.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.DungeonSaveData.MapObjectSaveData.ConcreteModel.ModuleMap.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.FoliageGridSaveDataMap.ModelMap.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.FoliageGridSaveDataMap.ModelMap.InstanceDataMap.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.MapObjectSaveData.Model.EffectMap.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.MapObjectSaveData.ConcreteModel.ModuleMap.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.CharacterSaveParameterMap.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.FoliageGridSaveDataMap.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.MapObjectSpawnerInStageSaveData.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
        ".worldSaveData.MapObjectSpawnerInStageSaveData.SpawnerDataMapByLevelObjectInstanceId.Key"
            .into(),
        uesave::StructType::Guid,
    );
        types.add(
        ".worldSaveData.MapObjectSpawnerInStageSaveData.SpawnerDataMapByLevelObjectInstanceId.Value".into(),
        uesave::StructType::Struct(None),
    );
        types.add(
            ".worldSaveData.BaseCampSaveData.Key".into(),
            uesave::StructType::Guid,
        );
        types.add(
            ".worldSaveData.BaseCampSaveData.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.BaseCampSaveData.ModuleMap.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.ItemContainerSaveData.Value".into(),
            uesave::StructType::Struct(None),
        );
        types.add(
            ".worldSaveData.EnemyCampSaveData.EnemyCampStatusMap.Value".into(),
            uesave::StructType::Struct(None),
        );

        types.into()
    });

#[derive(Debug, Clone, PartialEq)]
pub struct PalSave {
    pub compression_type: u8,
    pub save: Save,
}

pub fn read_save_file<R: Read>(reader: R) -> anyhow::Result<PalSave> {
    let mut reader = BufReader::new(reader);

    let _decompresed_length = reader.read_u32::<LittleEndian>()?;
    let _compressed_length = reader.read_u32::<LittleEndian>()?;

    const PLZ_MAGIC: [u8; 3] = [b'P', b'l', b'Z'];
    let mut magic = [0u8; 3];
    reader.read_exact(&mut magic)?;
    if magic != PLZ_MAGIC {
        return Err(anyhow::anyhow!("Invalid magic"));
    }

    let compression_type = reader.read_u8()?;
    let save = match compression_type {
        0x30 => Save::read_with_types(&mut reader, &SAVE_TYPES)?,
        0x31 => {
            let mut reader = flate2::bufread::ZlibDecoder::new(reader);
            Save::read_with_types(&mut reader, &SAVE_TYPES)?
        }
        0x32 => {
            let mut reader =
                flate2::read::ZlibDecoder::new(flate2::bufread::ZlibDecoder::new(reader));
            Save::read_with_types(&mut reader, &SAVE_TYPES)?
        }
        _ => return Err(anyhow::anyhow!("Invalid compression method")),
    };
    Ok(PalSave {
        compression_type,
        save,
    })
}

pub fn write_save_file<W: Write>(writer: &mut W, pal_save: &PalSave) -> anyhow::Result<()> {
    let mut uncompressed_save = Vec::new();
    pal_save.save.write(&mut uncompressed_save)?;

    let uncompressed_length = uncompressed_save.len() as u32;

    let mut compressor = Cursor::new(Vec::new());
    let compressed_length = match pal_save.compression_type {
        0x30 => {
            compressor.write_all(&uncompressed_save)?;
            uncompressed_length
        }
        0x31 => {
            let mut encoder =
                flate2::write::ZlibEncoder::new(&mut compressor, flate2::Compression::default());
            encoder.write_all(&uncompressed_save)?;
            encoder.finish()?;

            compressor.get_ref().len() as u32
        }
        0x32 => {
            let mut buffer = Cursor::new(Vec::new());
            let mut encoder =
                flate2::write::ZlibEncoder::new(&mut buffer, flate2::Compression::default());
            encoder.write_all(&uncompressed_save)?;
            encoder.finish()?;

            let mut buffer = buffer.into_inner();
            let compressed_length = buffer.len() as u32;

            let mut encoder =
                flate2::write::ZlibEncoder::new(&mut compressor, flate2::Compression::default());
            encoder.write_all(&mut buffer)?;

            compressed_length
        }
        _ => return Err(anyhow::anyhow!("Invalid compression method")),
    };

    let compressed = compressor.into_inner();

    writer.write_all(&uncompressed_length.to_le_bytes())?;
    writer.write_all(&compressed_length.to_le_bytes())?;
    writer.write_all(&[b'P', b'l', b'Z'])?;
    writer.write_all(&[pal_save.compression_type])?;
    writer.write_all(&compressed)?;

    Ok(())
}

pub fn get_world_save_data(pal_save: &PalSave) -> &IndexMap<String, uesave::Property> {
    let uesave::Property::Struct {
        value: uesave::StructValue::Struct(world_save_data),
        ..
    } = pal_save.save.root.properties.get("worldSaveData").unwrap()
    else {
        panic!()
    };
    world_save_data
}

pub fn get_world_save_data_mut(pal_save: &mut PalSave) -> &mut IndexMap<String, uesave::Property> {
    let uesave::Property::Struct {
        value: uesave::StructValue::Struct(world_save_data),
        ..
    } = pal_save
        .save
        .root
        .properties
        .get_mut("worldSaveData")
        .unwrap()
    else {
        panic!()
    };
    world_save_data
}

pub fn get_group_save_data_map(pal_save: &PalSave) -> &Vec<uesave::MapEntry> {
    let world_save_data = get_world_save_data(&pal_save);
    let uesave::Property::Map {
        value: group_save_data_map,
        ..
    } = world_save_data.get("GroupSaveDataMap").unwrap()
    else {
        panic!()
    };
    group_save_data_map
}

pub fn is_group_type_guild(entry: &uesave::MapEntry) -> bool {
    let uesave::PropertyValue::Struct(uesave::StructValue::Struct(ref value)) = entry.value else {
        panic!()
    };
    match &value["GroupType"] {
        uesave::Property::Enum { value: name, .. } => name == "EPalGroupType::Guild",
        _ => false,
    }
}

pub fn parse_raw_group_guild_save(entry: &uesave::MapEntry) -> GroupGuildSave {
    let uesave::PropertyValue::Struct(uesave::StructValue::Struct(ref value)) = entry.value else {
        panic!()
    };
    let uesave::Property::Array {
        value: uesave::ValueArray::Base(uesave::ValueVec::Byte(uesave::ByteArray::Byte(ref data))),
        ..
    } = &value["RawData"]
    else {
        panic!()
    };
    let binding = data.clone();
    let mut stream = stream(binding.as_slice());
    read_group_guild_save.parse_next(&mut stream).unwrap()
}

pub fn get_character_save_parameter_map(pal_save: &PalSave) -> &Vec<uesave::MapEntry> {
    let world_save_data = get_world_save_data(pal_save);
    let uesave::Property::Map {
        value: character_save_parameter_map,
        ..
    } = world_save_data.get("CharacterSaveParameterMap").unwrap()
    else {
        panic!()
    };
    character_save_parameter_map
}

pub fn get_character_save_parameter_map_mut(pal_save: &mut PalSave) -> &mut Vec<uesave::MapEntry> {
    let world_save_data = get_world_save_data_mut(pal_save);
    let uesave::Property::Map {
        value: character_save_parameter_map,
        ..
    } = world_save_data
        .get_mut("CharacterSaveParameterMap")
        .unwrap()
    else {
        panic!()
    };
    character_save_parameter_map
}

pub fn parse_raw_character_save_parameter<'a>(
    header: &'a uesave::Header,
) -> impl Fn(&'a uesave::MapEntry) -> CharacterSaveParameter {
    move |entry: &uesave::MapEntry| {
        let data = {
            let uesave::PropertyValue::Struct(uesave::StructValue::Struct(ref value)) = entry.value
            else {
                panic!()
            };
            let uesave::Property::Array {
                value:
                    uesave::ValueArray::Base(uesave::ValueVec::Byte(uesave::ByteArray::Byte(ref data))),
                ..
            } = &value["RawData"]
            else {
                panic!()
            };
            data
        };
        read_raw_character_save_parameter(&header)(data)
    }
}

#[test]
pub fn test_read_write_save_file() {
    use std::io::Cursor;

    let mut save = std::fs::read("assets/Level.sav").unwrap();
    let pal_save = read_save_file(Cursor::new(&mut save)).unwrap();

    let mut re_save = Vec::new();
    write_save_file(&mut re_save, &pal_save).unwrap();
    let re_pal_save = read_save_file(Cursor::new(&mut re_save)).unwrap();
    assert_eq!(pal_save, re_pal_save);
}
