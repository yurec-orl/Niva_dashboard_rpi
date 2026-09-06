#include "ds18b20_bus.h"

#include <Arduino.h>
#include <OneWire.h>

// ============================================================
// Configuration
// ============================================================

#define DS18B20_PIN          PA10   // 1-Wire bus GPIO, 4.7k pull-up to 3.3V
#define MAX_DS18B20          10     // device table size; also sizes the $T buffer
#define DS18B20_RES_BITS     10     // 187.5 ms conversion, 0.25 deg C resolution
#define DS18B20_FAMILY       0x28   // DS18B20 family code — ignore anything else on the bus

// Max conversion time: 750 ms at 12-bit, halving per bit dropped. +50 ms slack so a
// borderline-slow sensor is never read early.
#define DS18B20_CONV_TIME_MS ((750UL >> (12 - DS18B20_RES_BITS)) + 50)

// Config register byte: bit7 = 0, resolution R1:R0 at bits 6:5, bits 4:0 read as 1.
#define DS18B20_CFG_REG      (0x1F | (((DS18B20_RES_BITS) - 9) << 5))

// 1-Wire ROM / function commands
#define OW_CMD_CONVERT_T     0x44
#define OW_CMD_WRITE_SCRATCH 0x4E
#define OW_CMD_READ_SCRATCH  0xBE

// ============================================================
// Device table — populated once by OW_SEARCH
// ============================================================

struct Ds18b20Device {
    uint8_t rom[8];
    int16_t raw;              // last good temperature register value (1/16 deg C)
    bool    valid_this_cycle; // read good in the current convert->read cycle
};

static OneWire ow(DS18B20_PIN);

static Ds18b20Device devices[MAX_DS18B20];
static uint8_t        device_count = 0;

// ============================================================
// Non-blocking state machine
// ============================================================

enum OwState { OW_SEARCH, OW_CONVERT, OW_WAIT, OW_READ };
static OwState  ow_state = OW_SEARCH;
static uint8_t  read_index = 0;         // OW_READ: sensor polled this tick
static uint32_t convert_started_ms = 0;

// $T transmit buffer. Per ONEWIRE_TEMP_SENSOR_DESIGN.md: 16 hex + ':' + up to a
// 6-char signed value + ';' = 24 chars per device, plus "$T," and "\n".
static char temp_frame[MAX_DS18B20 * 24 + 16];

void ds18b20_setup() {
    device_count = 0;
    read_index = 0;
    ow_state = OW_SEARCH;
    ow.reset_search();
}

// SKIP ROM + WRITE SCRATCHPAD to put every device at DS18B20_RES_BITS resolution.
// Not committed to EEPROM (no COPY SCRATCHPAD) — the STM32 re-runs this on every boot,
// so surviving power loss buys nothing and the ~10 ms EEPROM write is avoided.
static void ds18b20_write_resolution() {
    if (!ow.reset()) return;           // no presence pulse — nothing to configure
    ow.skip();
    ow.write(OW_CMD_WRITE_SCRATCH);
    ow.write(0x00);                     // TH — alarm feature unused
    ow.write(0x00);                     // TL — alarm feature unused
    ow.write(DS18B20_CFG_REG);
}

// Build the $T line from every sensor that read good this cycle and send it. Zero
// good sensors still emits "$T\n" so the dashboard can tell "bus alive, nothing
// found" from "link down / firmware predates this feature".
static void ds18b20_send_frame() {
    int n = snprintf(temp_frame, sizeof(temp_frame), "$T");

    uint8_t valid = 0;
    for (uint8_t i = 0; i < device_count; i++) {
        if (devices[i].valid_this_cycle) valid++;
    }
    if (valid > 0) {
        n += snprintf(temp_frame + n, sizeof(temp_frame) - n, ",");
    }

    for (uint8_t i = 0; i < device_count; i++) {
        if (!devices[i].valid_this_cycle) continue;
        if (n >= (int)sizeof(temp_frame) - 28) break;   // defensive; buffer sizing prevents this
        for (uint8_t b = 0; b < 8; b++) {
            n += snprintf(temp_frame + n, sizeof(temp_frame) - n, "%02x", devices[i].rom[b]);
        }
        n += snprintf(temp_frame + n, sizeof(temp_frame) - n, ":%d;", (int)devices[i].raw);
    }

    snprintf(temp_frame + n, sizeof(temp_frame) - n, "\n");
    Serial.print(temp_frame);
}

void ds18b20_tick() {
    switch (ow_state) {

    // Run SEARCH ROM one device per tick (each call is a reset plus 64 read-triplets,
    // a few ms). Spreading it across ticks keeps boot-time telemetry frames on schedule.
    case OW_SEARCH: {
        uint8_t rom[8];
        if (ow.search(rom)) {
            if (OneWire::crc8(rom, 7) == rom[7] &&
                rom[0] == DS18B20_FAMILY &&
                device_count < MAX_DS18B20) {
                memcpy(devices[device_count].rom, rom, 8);
                devices[device_count].raw = 0;
                devices[device_count].valid_this_cycle = false;
                device_count++;
            }
            // stay in OW_SEARCH for the next device
        } else {
            ds18b20_write_resolution();
            ow_state = OW_CONVERT;      // search done — never return here
        }
        break;
    }

    // Kick off a conversion on every device at once (SKIP ROM + CONVERT T).
    case OW_CONVERT:
        ow.reset();
        ow.skip();
        ow.write(OW_CMD_CONVERT_T);
        convert_started_ms = millis();
        read_index = 0;
        ow_state = OW_WAIT;
        break;

    // No bus activity until the conversion completes. Timed off millis() (not a tick
    // count) so a blocking "$OSCCAP" capture mid-wait only lengthens it, harmlessly.
    case OW_WAIT:
        if (millis() - convert_started_ms >= DS18B20_CONV_TIME_MS) {
            if (device_count == 0) {
                ds18b20_send_frame();   // "$T\n"
                ow_state = OW_CONVERT;
            } else {
                ow_state = OW_READ;
            }
        }
        break;

    // One sensor per tick: MATCH ROM + READ SCRATCHPAD, CRC8 over the 9 bytes. A bad
    // read (e.g. a tacho/speed EXTI preempting a read slot) is simply omitted from this
    // cycle's $T line and retried next cycle — see ONEWIRE_TEMP_SENSOR_DESIGN.md. If
    // bench testing shows an unacceptable retry rate, mask EXTI0/EXTI1 around just this
    // burst rather than the whole transaction.
    case OW_READ: {
        Ds18b20Device &dev = devices[read_index];
        uint8_t sp[9];
        bool ok = false;
        if (ow.reset()) {
            ow.select(dev.rom);
            ow.write(OW_CMD_READ_SCRATCH);
            for (uint8_t i = 0; i < 9; i++) sp[i] = ow.read();
            ok = (OneWire::crc8(sp, 8) == sp[8]);
        }
        if (ok) {
            dev.raw = (int16_t)((sp[1] << 8) | sp[0]);
            dev.valid_this_cycle = true;
        } else {
            dev.valid_this_cycle = false;
        }

        read_index++;
        if (read_index >= device_count) {
            ds18b20_send_frame();
            ow_state = OW_CONVERT;
        }
        break;
    }
    }
}
