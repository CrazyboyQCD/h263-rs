//! Macroblock decoding

use crate::decoder::reader::H263Reader;
use crate::error::{Error, Result};
use crate::types::{CodedBlockPattern, Macroblock, MacroblockType, Picture, PictureTypeCode, PictureOption, HalfPel};
use crate::vlc::{Entry, Entry::End, Entry::Fork};
use std::io::Read;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BlockPatternEntry {
    Stuffing,

    Invalid,

    Valid(MacroblockType, bool, bool),
}

const MCBPC_I_TABLE: [Entry<BlockPatternEntry>; 21] = [
    Fork(2, 1), //x, slot 0
    End(BlockPatternEntry::Valid(
        MacroblockType::Intra,
        false,
        false,
    )), //1, slot 1
    Fork(6, 3), //0x, slot 2
    Fork(4, 5), //01x, slot 3
    End(BlockPatternEntry::Valid(MacroblockType::Intra, true, false)), //010, slot 4
    End(BlockPatternEntry::Valid(MacroblockType::Intra, true, true)), //011, slot 5
    Fork(8, 7), //00x, slot 6
    End(BlockPatternEntry::Valid(MacroblockType::Intra, false, true)), //001, slot 7
    Fork(10, 9), //000x, slot 8
    End(BlockPatternEntry::Valid(
        MacroblockType::IntraQ,
        false,
        false,
    )), //0001, slot 9
    Fork(14, 11), //0000x, slot 10
    Fork(12, 13), //00001x, slot 11
    End(BlockPatternEntry::Valid(
        MacroblockType::IntraQ,
        true,
        false,
    )), //000010, slot 12
    End(BlockPatternEntry::Valid(MacroblockType::IntraQ, true, true)), //000011, slot 13
    Fork(16, 20), //00000x, slot 14
    End(BlockPatternEntry::Invalid), //slot 15
    Fork(17, 15), //000000x, slot 16
    Fork(18, 15), //0000000x, slot 17
    Fork(15, 19), //00000000x, slot 18
    End(BlockPatternEntry::Stuffing), //000000001, slot 19
    End(BlockPatternEntry::Valid(
        MacroblockType::IntraQ,
        false,
        true,
    )), //000001, slot 20
];

const MCBPC_P_TABLE: [Entry<BlockPatternEntry>; 53] = [
    Fork(2, 1), //x, slot 0
    End(BlockPatternEntry::Valid(
        MacroblockType::Inter,
        false,
        false,
    )), //1, slot 1
    Fork(6, 3), //0x, slot 2
    Fork(4, 5), //01x, slot 3
    End(BlockPatternEntry::Valid(
        MacroblockType::Inter4V,
        false,
        false,
    )), //010, slot 4
    End(BlockPatternEntry::Valid(
        MacroblockType::InterQ,
        false,
        false,
    )), //011, slot 5
    Fork(10, 7), //00x, slot 6
    Fork(8, 9), //001x, slot 7
    End(BlockPatternEntry::Valid(MacroblockType::Inter, true, false)), //0010, slot 8
    End(BlockPatternEntry::Valid(MacroblockType::Inter, false, true)), //0011, slot 9
    Fork(16, 11), //000x, slot 10
    Fork(13, 12), //0001x, slot 11
    End(BlockPatternEntry::Valid(
        MacroblockType::Intra,
        false,
        false,
    )), //00011, slot 12
    Fork(14, 15), //00010x, slot 13
    End(BlockPatternEntry::Valid(
        MacroblockType::IntraQ,
        false,
        false,
    )), //000100, slot 14
    End(BlockPatternEntry::Valid(MacroblockType::Inter, true, true)), //000101, slot 15
    Fork(24, 17), //0000x, slot 16
    Fork(18, 21), //00001x, slot 17
    Fork(19, 20), //000010x, slot 18
    End(BlockPatternEntry::Valid(
        MacroblockType::Inter4V,
        true,
        false,
    )), //0000100, slot 19
    End(BlockPatternEntry::Valid(
        MacroblockType::Inter4V,
        false,
        true,
    )), //0000101, slot 20
    Fork(22, 23), //000011x, slot 21
    End(BlockPatternEntry::Valid(
        MacroblockType::InterQ,
        true,
        false,
    )), //0000110, slot 22
    End(BlockPatternEntry::Valid(
        MacroblockType::InterQ,
        false,
        true,
    )), //0000111, slot 23
    Fork(30, 25), //00000x, slot 24
    Fork(27, 26), //000001x, slot 25
    End(BlockPatternEntry::Valid(MacroblockType::Intra, true, true)), //0000011, slot 26
    Fork(28, 29), //0000010x, slot 27
    End(BlockPatternEntry::Valid(MacroblockType::Intra, false, true)), //00000100, slot 28
    End(BlockPatternEntry::Valid(
        MacroblockType::Inter4V,
        true,
        true,
    )), //00000101, slot 29
    Fork(36, 31), //000000x, slot 30
    Fork(33, 32), //0000001x, slot 31
    End(BlockPatternEntry::Valid(MacroblockType::Intra, true, false)), //00000011, slot 32
    Fork(34, 35), //00000010x, slot 33
    End(BlockPatternEntry::Valid(
        MacroblockType::IntraQ,
        false,
        true,
    )), //000000100, slot 34
    End(BlockPatternEntry::Valid(MacroblockType::InterQ, true, true)), //000000101, slot 35
    Fork(40, 37), //0000000x, slot 36
    Fork(38, 39), //00000001x, slot 37
    End(BlockPatternEntry::Valid(MacroblockType::IntraQ, true, true)), //000000010, slot 38
    End(BlockPatternEntry::Valid(
        MacroblockType::IntraQ,
        true,
        false,
    )), //000000011, slot 39
    Fork(42, 41), //00000000x, slot 40
    End(BlockPatternEntry::Stuffing), //000000001, slot 41
    Fork(43, 44), //000000000x, slot 42
    End(BlockPatternEntry::Invalid), //slot 43: no long runs of zeroes
    Fork(45, 46), //0000000001x, slot 44
    End(BlockPatternEntry::Valid(
        MacroblockType::Inter4VQ,
        false,
        false,
    )), //00000000010, slot 45
    Fork(47, 50), //00000000011x, slot 46
    Fork(48, 49), //000000000110x, slot 47
    End(BlockPatternEntry::Valid(
        MacroblockType::Inter4VQ,
        false,
        true,
    )), //0000000001100, slot 48
    End(BlockPatternEntry::Invalid), //0000000001101, slot 49
    Fork(51, 52), //000000000111x, slot 50
    End(BlockPatternEntry::Valid(
        MacroblockType::Inter4VQ,
        true,
        false,
    )), //0000000001110, slot 51
    End(BlockPatternEntry::Valid(
        MacroblockType::Inter4VQ,
        true,
        true,
    )), //0000000001111, slot 52
];

/// The decoding table for MODB (used in PB Frames).
/// 
/// The output of this table is two booleans, the first indicating the presence
/// of a `CodedBlockPattern` for the B-blocks, and the second indicating the
/// presence of motion vectors for the B-blocks.
const MODB_TABLE: [Entry<(bool, bool)>; 5] = [
    Fork(1, 2), //x, slot 0
    End((false, false)), //0, slot 1

    Fork(3, 4), //1x, slot 2
    End((false, true)), //10, slot 3
    End((true, true)), //11, slot 4
];

fn decode_cbpb<R>(
    reader: &mut H263Reader<R>,
) -> Result<CodedBlockPattern>
where
    R: Read,
{
    reader.with_transaction(|reader| {
        let cbp0 = reader.read_bits::<u8>(1)? == 1;
        let cbp1 = reader.read_bits::<u8>(1)? == 1;
        let cbp2 = reader.read_bits::<u8>(1)? == 1;
        let cbp3 = reader.read_bits::<u8>(1)? == 1;
        let cbp4 = reader.read_bits::<u8>(1)? == 1;
        let cbp5 = reader.read_bits::<u8>(1)? == 1;
        
        Ok(CodedBlockPattern {
            codes_luma: [cbp0, cbp1, cbp2, cbp3],
            codes_chroma_b: cbp4,
            codes_chroma_r: cbp5,
        })
    })
}

/// The coding table for all four possible luma block codes.
/// 
/// This specifically produces the INTRA block pattern; for the INTER block
/// pattern, flip all bits in the result. A `None` indicates an invalid
/// bitstream.
const CBPY_TABLE_INTRA: [Entry<Option<[bool; 4]>>; 33] = [
    Fork(1, 24), //x, slot 0

    Fork(2, 17), //0x, slot 1

    Fork(3, 12), //00x, slot 2

    Fork(4, 9), //000x, slot 3

    Fork(5, 6), //0000x, slot 4
    End(None), //00000, slot 5, not a valid prefix

    Fork(7, 8), //00001x, slot 6
    End(Some([false, true, true, false])), //000010, slot 7
    End(Some([true, false, false, true])), //000011, slot 8

    Fork(10, 11), //0001, slot 9
    End(Some([true, false, false, false])), //00010, slot 10
    End(Some([false, true, false, false])), //00011, slot 11

    Fork(13, 16), //001x, slot 12

    Fork(14, 15), //0010x, slot 13
    End(Some([false, false, true, false])), //00100, slot 14
    End(Some([false, false, false, true])), //00101, slot 15

    End(Some([false, false, false, false])), //0011, slot 16

    Fork(18, 21), //01x, slot 17

    Fork(19, 20), //010x, slot 18
    End(Some([true, true, false, false])), //0100, slot 19
    End(Some([true, false, true, false])), //0101, slot 20

    Fork(22, 23), //011x, slot 21
    End(Some([true, true, true, false])), //0110, slot 22
    End(Some([false, true, false, true])), //0111, slot 23

    Fork(25, 32), //1x, slot 24

    Fork(26, 29), //10x, slot 25

    Fork(27, 28), //100x, slot 26
    End(Some([true, true, false, true])), //1000, slot 27
    End(Some([false, false, true, true])), //1001, slot 28

    Fork(30, 31), //101x, slot 29
    End(Some([true, false, true, true])), //1010, slot 30
    End(Some([false, true, true, true])), //1011, slot 31

    End(Some([true, true, true, true])), //11, slot 32
];

fn decode_dquant<R>(
    reader: &mut H263Reader<R>,
) -> Result<i8>
where
    R: Read,
{
    reader.with_transaction(|reader| {
        Ok(match reader.read_bits::<u8>(2)? {
            0 => -1,
            1 => -2,
            2 => 1,
            3 => 2,
            _ => return Err(Error::InternalDecoderError)
        })
    })
}
fn decode_macroblock_header<R>(
    reader: &mut H263Reader<R>,
    picture: &Picture,
) -> Result<Option<Macroblock>>
where
    R: Read,
{
    reader.with_transaction(|reader| {
        let is_coded: u8 = if matches!(picture.picture_type, PictureTypeCode::IFrame) {
            0
        } else {
            reader.read_bits(1)?
        };

        if is_coded == 0 {
            let mcbpc = match picture.picture_type {
                PictureTypeCode::IFrame => reader.read_vlc(&MCBPC_I_TABLE[..])?,
                PictureTypeCode::PFrame => reader.read_vlc(&MCBPC_P_TABLE[..])?,
                _ => return Err(Error::UnimplementedDecoding),
            };

            let (mbt, chroma_b, chroma_r) = match mcbpc {
                BlockPatternEntry::Stuffing => return Ok(Some(Macroblock::Stuffing)),
                BlockPatternEntry::Invalid => return Err(Error::InvalidBitstream),
                BlockPatternEntry::Valid(mbt, chroma_b, chroma_r) => (mbt, chroma_b, chroma_r),
            };

            let (has_cbpb, has_mvdb) = if matches!(picture.picture_type, PictureTypeCode::PBFrame) {
                reader.read_vlc(&MODB_TABLE[..])?
            } else {
                (false, false)
            };

            let coded_block_pattern_b = if has_cbpb {
                Some(decode_cbpb(reader)?)
            } else {
                None
            };

            let coded_block_pattern_luma = if matches!(mbt, MacroblockType::Intra) || matches!(mbt, MacroblockType::IntraQ) {
                match reader.read_vlc(&CBPY_TABLE_INTRA)? {
                    Some(v) => v,
                    None => return Err(Error::InvalidBitstream),
                }
            } else {
                match reader.read_vlc(&CBPY_TABLE_INTRA)? {
                    Some([v1, v2, v3, v4]) => [!v1, !v2, !v3, !v4],
                    None => return Err(Error::InvalidBitstream),
                }
            };

            let d_quantizer = if picture.options.contains(PictureOption::ModifiedQuantization) {
                return Err(Error::UnimplementedDecoding);
            } else if matches!(mbt, MacroblockType::InterQ) || matches!(mbt, MacroblockType::IntraQ) || matches!(mbt, MacroblockType::Inter4VQ) {
                Some(decode_dquant(reader)?)
            } else {
                None
            };

            //Ok(Some(Macroblock::Coded {}))
            Err(Error::UnimplementedDecoding)
        } else {
            Ok(Some(Macroblock::Uncoded))
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::decoder::macroblock::{BlockPatternEntry, MCBPC_I_TABLE, MCBPC_P_TABLE, MODB_TABLE, CBPY_TABLE_INTRA};
    use crate::decoder::reader::H263Reader;
    use crate::types::MacroblockType;

    #[test]
    #[allow(clippy::inconsistent_digit_grouping)]
    fn macroblock_mcbpc_iframe() {
        let bit_pattern = vec![
            0b1_001_010_0,
            0b11_0001_00,
            0b0001_0000,
            0b10_000011,
            0b00000000,
            0b1_0000001,
        ];
        let mut reader = H263Reader::from_source(&bit_pattern[..]);

        assert_eq!(
            reader.read_vlc(&MCBPC_I_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Intra, false, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_I_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Intra, false, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_I_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Intra, true, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_I_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Intra, true, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_I_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::IntraQ, false, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_I_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::IntraQ, false, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_I_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::IntraQ, true, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_I_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::IntraQ, true, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_I_TABLE).unwrap(),
            BlockPatternEntry::Stuffing
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_I_TABLE).unwrap(),
            BlockPatternEntry::Invalid
        );
    }

    #[test]
    #[allow(clippy::inconsistent_digit_grouping)]
    fn macroblock_mcbpc_pframe() {
        let bit_pattern = vec![
            0b1_0011_001,
            0b0_000101_0,
            0b11_000011,
            0b1_0000110,
            0b00000010,
            0b1_010_0000,
            0b101_00001,
            0b00_000001,
            0b01_00011_0,
            0b0000100_0,
            0b0000011_0,
            0b000011_00,
            0b0100_0000,
            0b00100_000,
            0b000011_00,
            0b0000010_0,
            0b00000001,
            0b00000000,
            0b010_00000,
            0b00001100,
            0b00000000,
            0b01110_000,
            0b00000011,
            0b11_000000,
            0b00000000,
        ];
        let mut reader = H263Reader::from_source(&bit_pattern[..]);

        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Inter, false, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Inter, false, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Inter, true, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Inter, true, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::InterQ, false, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::InterQ, false, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::InterQ, true, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::InterQ, true, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Inter4V, false, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Inter4V, false, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Inter4V, true, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Inter4V, true, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Intra, false, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Intra, false, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Intra, true, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Intra, true, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::IntraQ, false, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::IntraQ, false, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::IntraQ, true, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::IntraQ, true, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Stuffing
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Inter4VQ, false, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Inter4VQ, false, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Inter4VQ, true, false)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Valid(MacroblockType::Inter4VQ, true, true)
        );
        assert_eq!(
            reader.read_vlc(&MCBPC_P_TABLE).unwrap(),
            BlockPatternEntry::Invalid
        );
    }

    #[test]
    #[allow(clippy::inconsistent_digit_grouping)]
    fn macroblock_modb_table() {
        let bit_pattern = vec![
            0b0_10_11_000,
        ];
        let mut reader = H263Reader::from_source(&bit_pattern[..]);

        assert_eq!(reader.read_vlc(&MODB_TABLE).unwrap(), (false, false));
        assert_eq!(reader.read_vlc(&MODB_TABLE).unwrap(), (false, true));
        assert_eq!(reader.read_vlc(&MODB_TABLE).unwrap(), (true, true));
    }

    #[test]
    #[allow(clippy::inconsistent_digit_grouping)]
    fn macroblock_cbpy_table() {
        let bit_pattern = vec![
            0b0011_0010,
            0b1_00100_10,
            0b01_00011_0,
            0b111_00001,
            0b0_1011_000,
            0b10_000011,
            0b0101_1010,
            0b0100_1000,
            0b0110_11_00,
            0b00_0000_00,
        ];
        let mut reader = H263Reader::from_source(&bit_pattern[..]);

        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([false, false, false, false]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([false, false, false, true]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([false, false, true, false]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([false, false, true, true]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([false, true, false, false]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([false, true, false, true]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([false, true, true, false]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([false, true, true, true]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([true, false, false, false]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([true, false, false, true]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([true, false, true, false]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([true, false, true, true]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([true, true, false, false]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([true, true, false, true]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([true, true, true, false]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), Some([true, true, true, true]));
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), None);
        assert_eq!(reader.read_vlc(&CBPY_TABLE_INTRA).unwrap(), None);
    }

}