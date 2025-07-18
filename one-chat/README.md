A uart, spi, and radio trace of a "hello world" level chat.

Order of events:
 * shinelan uarts from the mainboard to the daughterboard:
   `R F 9 [01] [00] [10] K W K 1 C G Q 1 1 A H Z L 0 C G Q 1 1 A [03] [00] [00] [16] [14]`
 * daughterboard does nothing for 10ms, then spends ~15ms brining up the radio
 * daughterboard spis to the radio board:
   `FF 15 14 76 56 41 44 1F 05 0D 1F 04 15 1E 66 70 15 1C 08 0A 1E 04 15 1E 66 70 15 57 52 46 38 53`
    * if you xor this packet with (ascii):
      `F.GROWATTRF.GROWATTRF.GROWATTRF.G`
    * you end up with:
      `..RF9. .KWK1CGQ11AHZL0CGQ11A.  ..`
    * I don't know why it starts at the first/second byte (brute force).
 * 176ms pass, including setting `87 01` and `8E 02` on the radio
 * radio board spis to the daughterboard:
   `00 15 14 76 56 41 44 1F 05 0D 1F 04 15 1E 66 70 15 1C 08 0A 1E 04 15 1E 66 70 15 57 52 47 2C 19 C4`
   *  key: `.GROWATTRF.GROWATTRF.GROWATTRF.GRO`, data: `.RF9. .KWK1CGQ11AHZL0CGQ11A. ..^..`
 * daughterboard uarts to the mainboard:
   `R F 9 [01] [00] [10] K W K 1 C G Q 1 1 A H Z L 0 C G Q 1 1 A [03] [00] [01] [02] ^ [96]`

---

In this packet, the `67` (0x10 unencrypted) at position ~4 appears to be a sequence number:
values observed between 0x11 and 0xfc (encrypted).

The decrypted data appears to have a crc16/modbus on the end.
Checked once with https://crccalc.com/ but have lost it.
None of the rest of the packets look like any modbus I can source.

CRC covers from `0x1476` (decrypted: `0x5246` (`RF`)) through to the supposed data:
`[03] [00] [01] [02]`.

Now wondering if all the partial captures are actually bugs:
it's actually retransmitting nonsense because of bad buffer hygiene.

e.g. this packet:
001514FC5641441F050D1F04151E6670151C080A1E04151E66701542527A661D1E7F1406056563072E47524F5741545452462E

decrypts as:
`F;S... .KWK1CGQ11AHZL0CGQ11A. <HZL0CGQ11A      ` (with many trailing nulls)

That could be a normal packet up to the end of the serials, then a crc (then end of the packet), then some random
left over data in the rest of the buffer? Surely not.

---

Shinelink serial: KWK1CGQ11A

ShineRF-S serial: HL0CGQ11A
