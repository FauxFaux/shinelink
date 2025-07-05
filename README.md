## ShineLink

ShineLink is an apparently deprecated Growatt inverter monitoring standard.

ShineLink-X (modbus over usb?) or ShineLink-S (modbus over serial?) connect to the inverter,
transmit data, and a ShineLanBox receives(?) this data and uploads it to the cloud.

This is different to the ShineLan-X and the ShineWifi-S (and e.g. the 4g variants), as there
is a mysterious link between the ShineLanBox and the ShineLink.

Projects like [grott](https://github.com/johanmeijer/grott) interfere with the cloud communication.

Projects like [OpenInverterGateway](https://github.com/OpenInverterGateway/OpenInverterGateway) replace the firmware
on a ShineWifi-S.

### ShineLanBox

[fccid](https://fccid.io/2AAJ9-SHINELANBOX)

Reasonably strong evidence that this is a 433MHz ISM-band device.

Board photo:
![ShineLanBox board](docs/board-1.jpg)

Mystery QR code in an unpopulated chip position redacted.

 * U1 (power side): D9329 2063 - some buck regulator (odd as they have a 5V/1A barrel jack in)
 * U2: ?, guessing main MCU
 * U15 (ethernet side): ENC28J60-?/?? (83) 2231A?0 Microchip - SPI ethernet controller


### 433 daughterboard

daughterdaughterboard:
 * SI4432 BPS1K5 2141 - Si4432 revision B, [internal], 2021 week 41 
 * 6-pin chip labelled "100"
 * crystal labelled "JHF 30.000"

Visually very similar to the [G-NiceRF RF4432](docs/nicerf-4432.pdf)

daughterdaughterboard would hence be SDO/SDI/SCLK/nSEL (pins 6/7/8/9) (or gpio on 2/3/4)?
Pin 1 is opposite the antenna, at the top of the board, pin 12 at the bottom left.
1, 12, 13: gnd, 5: 3.3V, 10: interupt, 11: shutdown, 14: antenna.

Someone's managed to identify this chip as likely a STM8S003F3P6:
![board photo](docs/daughter-1.jpg)

Regrettably a full microcontroller, so could be doing anything.

Central microcontroller must have a spare serial interface. Only appears to have passive supports.

Appears to be connected to the GPIO (pin 3 track is clear),
and the serial lines go socmewhere (throughole).

Don't recognise the `530.0034400` marking. LED isn't exposed outside the case.
Similar marking on the main board, implying it's a custom part by the same company.
This makes it quite likely the main processor is an STM too.


### Next steps:

* on a sacrificial receiver: lever up the daughterboard, and see if it has any useful markings.
* guess the gpio pins on the daughterdaughter are for data, and spy on them and radio 
  transmissions to work out the encoding.
* work out which of the 5 pins on the daughterboard are what, and spy on the presumably spi/i²c happening there.
* re-try reading the signal pulled from the air, with the new information it probably
  doesn't have anything crazy going on (e.g. it's not lora, no built-in support for any modulation/encoding).
