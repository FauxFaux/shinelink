const MODBUS: crc::Crc<u16> = crc::Crc::<u16>::new(&crc::CRC_16_MODBUS);

pub fn crc_suffixed(data: &[u8]) -> Option<&[u8]> {
    if data.len() < 2 {
        return None;
    }

    let (input, checksum) = data.split_at(data.len() - 2);
    let expected = u16::from_be_bytes([checksum[0], checksum[1]]);
    if MODBUS.checksum(input) == expected {
        Some(input)
    } else {
        None
    }
}
