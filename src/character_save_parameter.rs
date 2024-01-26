use std::io::{Cursor, Read};

use serde::{Deserialize, Serialize};

use byteorder::ReadBytesExt;
use indexmap::IndexMap;
use uuid::Uuid;
use winnow::Parser;

use crate::group_guild::{read_uuid, stream, write_uuid};

#[derive(Debug, Serialize, Deserialize)]
pub struct CharacterSaveParameter {
    pub properties: IndexMap<String, uesave::Property>,
    pub group_id: Uuid,
}

pub fn read_raw_character_save_parameter<'a>(
    header: &'a uesave::Header,
) -> impl Fn(&'a [u8]) -> CharacterSaveParameter {
    move |bytes: &[u8]| {
        let mut reader = Cursor::new(bytes);
        let properties = uesave::Context::run(&mut reader, |reader| {
            reader.header(&header, uesave::read_properties_until_none)
        })
        .unwrap();
        let _unknown = reader.read_u32::<byteorder::LittleEndian>().unwrap();
        let mut bytes = [0; 16];
        reader.read_exact(&mut bytes).unwrap();
        let uuid = read_uuid.parse_next(&mut stream(&bytes)).unwrap();
        CharacterSaveParameter {
            properties,
            group_id: uuid,
        }
    }
}

pub fn write_raw_character_save_parameter<'a>(
    header: &'a uesave::Header,
) -> impl Fn(&'a CharacterSaveParameter) -> Vec<u8> {
    move |character_save_parameter: &CharacterSaveParameter| {
        let mut bytes = Vec::new();
        uesave::Context::run(&mut bytes, |writer| {
            writer.header(&header, |writer| {
                uesave::write_properties_none_terminated(
                    writer,
                    &character_save_parameter.properties,
                )
            })
        })
        .unwrap();
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        bytes.extend_from_slice(&write_uuid(&character_save_parameter.group_id));
        bytes
    }
}

#[test]
pub fn test_read_write_character_save_parameter() {
    use std::io::Read;

    let header = uesave::Header {
        magic: Default::default(),
        save_game_version: Default::default(),
        package_version: uesave::PackageVersion::Old(0),
        engine_version_major: 5,
        engine_version_minor: Default::default(),
        engine_version_patch: Default::default(),
        engine_version_build: Default::default(),
        engine_version: Default::default(),
        custom_format_version: Default::default(),
        custom_format: Default::default(),
    };

    let file = std::fs::File::open("assets/character_save_parameter.bin").unwrap();
    let mut reader = std::io::BufReader::new(file);
    let mut data = Vec::new();
    reader.read_to_end(&mut data).unwrap();

    let character_save_parameter =
        read_raw_character_save_parameter(&header)(&mut stream(data.as_ref()));
    let data2 = write_raw_character_save_parameter(&header)(&character_save_parameter);
    assert_eq!(data, data2.as_slice());
}
