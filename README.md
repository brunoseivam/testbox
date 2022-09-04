TestBox
=======

## Electronics diagram

```
                 _______________D1 mini_______________
                |                                     |
                | 1  RST                 /GPIO1 TX 22 |
                | 2  A0 ADC0              GPIO3 RX 21 |
                | 4  D0 GPIO16        SCL GPIO5 D1 20 |
Servo Motor <-- | 5  D5 GPIO14 SCK    SDA GPIO4 D2 19 |
  Green LED <-- | 6  D6 GPIO12 MISO       GPIO0 D3 18 | --> Red LED
 Yellow LED <-- | 7  D7 GPIO13 MOSI       GPIO2 D4 17 | --> DHT22 temp sensor
                | 16 D8 GPIO15 SS              GND 15 |
                | 8  3V3                       5V USB |
                |_____________________________________|
```

## Protocol

Serial protocol. Baud rate: 115200.

### Syntax

* Line ending: `\n`
* Request: `<Verb> <Noun> <Value>`
* Good response: `OK <response>`
* Error response: `ERR <error code>`


### Error codes

| Error Code   | Description                                               |
|--------------|-----------------------------------------------------------|
| `BAD_SYNTAX` | Command is syntatically incorrect                         |
| `BAD_VERB`   | Provided verb is invalid                                  |
| `BAD_NOUN`   | Provided noun is invalid or verb does not accept a noun   |
| `BAD_VALUE`  | Provided value is invalid or verb does not accept a value |

### Commands

#### `ID`

Accepts no nouns and no values. Returns an identification ID for the board.

**Example:**

| Request | Response                  |
|---------|---------------------------|
| `ID`    | `OK ESP8266_WEMOS_D1MINI` |

#### `GET`

Accepts a noun and no values. Returns the noun's corresponding reading.
Valid nouns:

* `RED_LED`: intensity of the red LED, between `0` and `1023`.
* `YELLOW_LED`: intensity of the yellow LED, between `0` and `1023`.
* `GREEN_LED`: intensity of the green LED, between `0` and `1023`.
* `SERVO`: angle of the servo motor, between `0` and `180`, in degrees.
* `TEMP_AND_HUM`: sensor state, temperature and humidity reading. Temperature in Celsius, humidity in %.
* `SELF_TEST`: whether self test is in progress (`ACTIVE` or `INACTIVE`) and the progress percentage, if active.

**Examples:**

| Request            | Response                  | Notes
|--------------------|---------------------------|------
| `GET RED_LED`      | `OK 1000`                 |
| `GET SERVO`        | `OK 90`                   |
| `GET TEMP_AND_HUM` | `OK OK 29.90 55.20`       | First `OK` is the command response. Second `OK` is sensor reading status
| `GET TEMP_AND_HUM` | `OK CHECKSUM 0.00 0.00`   | `CHECKSUM` means there was a failure to read the sensor. In this case, both readings are zero
| `GET SELF_TEST`    | `OK INACTIVE 0`           | Self test is not in progress
| `GET SELF_TEST`    | `OK ACTIVE 50`            | Self test is in progress, and it is 50% of the way through

#### `SET`

Accepts a noun and an integer value. Sets the noun to the given value. Returns the noun's corresponding reading.
Valid nouns:

* `RED_LED`: intensity of the red LED, between `0` and `1023`.
* `YELLOW_LED`: intensity of the yellow LED, between `0` and `1023`.
* `GREEN_LED`: intensity of the green LED, between `0` and `1023`.
* `SERVO`: angle of the servo motor, between `0` and `180`, in degrees.
* `SELF_TEST`: `1` -> initiate the self test routine or `0` -> stops ongoing self test

**Examples:**

| Request            | Response                  | Notes
|--------------------|---------------------------|------
| `SET RED_LED 1000` | `OK 1000`                 |
| `SET RED_LED 2000` | `OK 1023`                 | Values over the max are clamped to max
| `SET RED_LED -100` | `OK 0`                    | Values under the min are clamped to min
| `SET SERVO 90`     | `OK 90`                   |
| `SET SELF_TEST 1`  | `OK ACTIVE 0`             |
| `SET SELF_TEST 0`  | `OK INACTIVE 0`           |
