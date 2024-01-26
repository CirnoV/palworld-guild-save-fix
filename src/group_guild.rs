#![allow(non_snake_case)]

use uuid::Uuid;
use winnow::{
    binary::{le_i32, le_u32, le_u64, le_u8, length_repeat},
    combinator::terminated,
    seq,
    token::take,
    trace::trace,
    Bytes, PResult, Parser, Partial,
};

pub type Stream<'i> = Partial<&'i Bytes>;

pub fn stream(bytes: &[u8]) -> Stream<'_> {
    Partial::new(Bytes::new(bytes))
}

pub fn read_uuid(s: &mut Stream) -> PResult<Uuid> {
    trace("Uuid", |i: &mut Stream| {
        let b = take(16usize).parse_next(i)?;
        Ok(uuid::Uuid::from_bytes([
            b[0x3], b[0x2], b[0x1], b[0x0], b[0x7], b[0x6], b[0x5], b[0x4], b[0xb], b[0xa], b[0x9],
            b[0x8], b[0xf], b[0xe], b[0xd], b[0xc],
        ]))
    })
    .parse_next(s)
}

pub fn write_uuid(guid: &Uuid) -> Vec<u8> {
    let b = guid.as_bytes();
    vec![
        b[0x3], b[0x2], b[0x1], b[0x0], b[0x7], b[0x6], b[0x5], b[0x4], b[0xb], b[0xa], b[0x9],
        b[0x8], b[0xf], b[0xe], b[0xd], b[0xc],
    ]
}

pub fn read_fstring(s: &mut Stream) -> PResult<String> {
    trace("FString", move |i: &mut Stream| {
        let len = le_i32.parse_next(i)?;
        if len == 0 {
            return Ok("".to_string());
        }

        let is_unicode = len < 0;
        if is_unicode {
            let len = -len as usize;
            trace(
                "Unicode",
                terminated(
                    take((len - 1) * 2).map(|s: &[u8]| {
                        String::from_utf16_lossy(
                            s.chunks(2)
                                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                                .collect::<Vec<_>>()
                                .as_slice(),
                        )
                    }),
                    b"\0\0",
                ),
            )
            .parse_next(i)
        } else {
            let len = len as usize;
            trace(
                "Non-Unicode",
                terminated(
                    take(len - 1).map(|s: &[u8]| String::from_utf8_lossy(s).to_string()),
                    b"\0",
                ),
            )
            .parse_next(i)
        }
    })
    .parse_next(s)
}

pub fn write_fstring(s: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    let is_unicode = s.len() != s.chars().count();
    if is_unicode {
        let utf16: Vec<u16> = s.encode_utf16().collect();
        let (_, aligned, _) = unsafe { utf16.align_to::<u8>() };
        bytes.extend_from_slice(&(-(aligned.len() as i32 / 2) - 1).to_le_bytes());
        bytes.extend_from_slice(aligned);
        bytes.extend_from_slice(&[0, 0]);
    } else {
        bytes.extend_from_slice(&(s.len() as i32 + 1).to_le_bytes());
        bytes.extend_from_slice(s.as_bytes());
        bytes.push(0);
    }
    bytes
}

#[derive(Debug, Clone, Copy)]
pub struct FDateTime {
    Ticks: u64,
}

pub fn read_fdatetime(s: &mut Stream) -> PResult<FDateTime> {
    trace(
        "FDateTime",
        seq! {
            FDateTime {
                Ticks: le_u64,
            }
        },
    )
    .parse_next(s)
}

pub fn write_fdatetime(datetime: &FDateTime) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&datetime.Ticks.to_le_bytes());
    bytes
}

#[derive(Debug, Clone, Copy)]
pub struct FPalInstanceId {
    PlayerUId: Uuid,
    InstanceUId: Uuid,
}

pub fn read_fpal_instance_id(s: &mut Stream) -> PResult<FPalInstanceId> {
    trace(
        "FPalInstanceId",
        seq! {
            FPalInstanceId {
                PlayerUId: read_uuid,
                InstanceUId: read_uuid,
            }
        },
    )
    .parse_next(s)
}

pub fn write_fpal_instance_id(instance_id: &FPalInstanceId) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&write_uuid(&instance_id.PlayerUId));
    bytes.extend_from_slice(&write_uuid(&instance_id.InstanceUId));
    bytes
}

#[derive(Debug, Clone)]
pub struct FPalGuildPlayerInfo {
    pub PlayerUId: Uuid,
    pub LastOnlineRealTime: FDateTime,
    pub PlayerName: String,
}

pub fn read_fpal_guild_player_info(s: &mut Stream) -> PResult<FPalGuildPlayerInfo> {
    trace(
        "FPalGuildPlayerInfo",
        seq! {
            FPalGuildPlayerInfo {
                PlayerUId: read_uuid,
                LastOnlineRealTime: read_fdatetime,
                PlayerName: read_fstring,
            }
        },
    )
    .parse_next(s)
}

pub fn write_fpal_guild_player_info(player_info: &FPalGuildPlayerInfo) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&write_uuid(&player_info.PlayerUId));
    bytes.extend_from_slice(&write_fdatetime(&player_info.LastOnlineRealTime));
    bytes.extend_from_slice(&write_fstring(&player_info.PlayerName));
    bytes
}

#[derive(Debug, Clone)]
pub struct GroupGuildSave {
    pub UnknownUuid: Uuid,
    pub MayBeOwner: String,
    pub InstanceIds: Vec<FPalInstanceId>,
    pub unknown: u8,
    pub UnknownGuid: Vec<Uuid>,
    pub BaseCampLevel: u32,
    pub UnknownGuid2: Vec<Uuid>,
    pub GuildName: String,
    pub AdminPlayerUId: Uuid,
    pub GuildPlayerInfo: Vec<FPalGuildPlayerInfo>,
}

pub fn read_group_guild_save(s: &mut Stream) -> PResult<GroupGuildSave> {
    trace(
        "GroupGuildSave",
        seq! {
            GroupGuildSave {
                UnknownUuid: read_uuid,
                MayBeOwner: read_fstring,
                InstanceIds: length_repeat(le_u32, read_fpal_instance_id),
                unknown: le_u8,
                UnknownGuid: length_repeat(le_u32, read_uuid),
                BaseCampLevel: le_u32,
                UnknownGuid2: length_repeat(le_u32, read_uuid),
                GuildName: read_fstring,
                AdminPlayerUId: read_uuid,
                GuildPlayerInfo: length_repeat(le_u32, read_fpal_guild_player_info),
            }
        },
    )
    .parse_next(s)
}

pub fn write_tarray<T, F>(items: &[T], write_item: F) -> Vec<u8>
where
    F: Fn(&T) -> Vec<u8>,
{
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(items.len() as u32).to_le_bytes());
    bytes.extend_from_slice(
        &items
            .iter()
            .flat_map(|item| write_item(item))
            .collect::<Vec<_>>(),
    );
    bytes
}

pub fn write_group_guild_save(group_guild_save: &GroupGuildSave) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&write_uuid(&group_guild_save.UnknownUuid));
    bytes.extend_from_slice(&write_fstring(&group_guild_save.MayBeOwner));
    bytes.extend_from_slice(&write_tarray(
        &group_guild_save.InstanceIds,
        write_fpal_instance_id,
    ));
    bytes.push(group_guild_save.unknown);
    bytes.extend_from_slice(&write_tarray(&group_guild_save.UnknownGuid, write_uuid));
    bytes.extend_from_slice(&group_guild_save.BaseCampLevel.to_le_bytes());
    bytes.extend_from_slice(&write_tarray(&group_guild_save.UnknownGuid2, write_uuid));
    bytes.extend_from_slice(&write_fstring(&group_guild_save.GuildName));
    bytes.extend_from_slice(&write_uuid(&group_guild_save.AdminPlayerUId));
    bytes.extend_from_slice(&write_tarray(
        &group_guild_save.GuildPlayerInfo,
        write_fpal_guild_player_info,
    ));
    bytes
}

#[test]
pub fn test_read_write_group_guild_save() {
    use std::io::Read;

    let file = std::fs::File::open("assets/guild_0.bin").unwrap();
    let mut reader = std::io::BufReader::new(file);
    let mut data = Vec::new();
    reader.read_to_end(&mut data).unwrap();

    let group_guild_save = read_group_guild_save(&mut stream(data.as_ref())).unwrap();
    let data2 = write_group_guild_save(&group_guild_save);
    assert_eq!(data, data2.as_slice());
}
