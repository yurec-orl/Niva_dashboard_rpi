// ============================================================
// Niva Dashboard — STM32 ADC/Sensor Module
// ============================================================
//
// Replaces Arduino Mega as the sensor acquisition module.
// Reads analog sensors, digital indicators, pulse signals, and K-Line,
// then sends a unified data frame over USB-serial to Raspberry Pi at 50 Hz.
//
// Target MCU: STM32F103C8T6 ("Blue Pill"), 72 MHz, 3.3V logic
//
// Protocol (ASCII, same format as Arduino version):
//   "$A0,A1,A2,A3,TACHO,SPEED,D0..D9,B0..B7\n"
//   - A0..A3:  raw 12-bit analog values (0-4095)
//   - TACHO:   pulse count since last report (tachometer, 2 PPR)
//   - SPEED:   pulse count since last report (speed sensor, 4 PPR)
//   - D0..D9:  digital indicator states (0/1)
//   - B0..B7:  button states (0/1, 1 = pressed)
//
// NOTE: All 12V car signals MUST go through appropriate voltage dividers
//       or level shifters before reaching the 3.3V STM32 pins.
//       K-Line uses an L9637D adapter (12V↔5V) + BSS138 level shifter (5V↔3.3V).
//
// ============================================================
// Pin Mapping — STM32F103C8T6
// ============================================================
//
// === Analog Inputs (12-bit ADC, 0-3.3V, via voltage dividers) ===
//
//   PA0 (ADC1_CH0) — Oil pressure sensor (analog)
//   PA1 (ADC1_CH1) — Fuel level sensor (analog)
//   PA2 (ADC1_CH2) — Coolant temperature sensor (analog)
//   PA3 (ADC1_CH3) — 12V system voltage (analog, via divider)
//
// === Pulse/Counter Inputs (interrupt-capable) ===
//
//   PB0  (EXTI0) — Tachometer signal, 2 pulses per revolution
//   PB1  (EXTI1) — Speed sensor signal, 4 pulses per revolution
//
//   Using external interrupts (EXTI) for pulse counting.
//   12V sensor signals go through voltage divider + 1nF filter cap to 3.3V.
//
// === Digital Indicator Inputs ===
//
//   PA8  — Oil pressure low warning        (INPUT_PULLUP, active-low)
//   PA9  — Fuel low warning                (INPUT_PULLUP, active-low)
//   PB3  — Charging indicator              (INPUT_PULLUP, active-low)
//   PB4  — Exterior lights on              (INPUT, active-high)
//   PB5  — Brake fluid low                 (INPUT, active-high)
//   PB6  — Headlights on                   (INPUT, active-high)
//   PB7  — Turn signal on                  (INPUT, active-high)
//   PB8  — High beams on                   (INPUT, active-high)
//   PB9  — Parking brake on                (INPUT_PULLUP, active-low)
//   PA15 — Diff lock on                    (INPUT_PULLUP, active-low)
//
//   All 12V-level digital signals need external level conversion to 3.3V.
//
// === Dashboard Buttons (active-low, internal pull-up) ===
//
//   PB12 — Button 0 (left column, top)
//   PB13 — Button 1 (left column, 2nd)
//   PB14 — Button 2 (left column, 3rd)
//   PB15 — Button 3 (left column, bottom)
//   PA4  — Button 4 (right column, top)
//   PA5  — Button 5 (right column, 2nd)
//   PA6  — Button 6 (right column, 3rd)
//   PA7  — Button 7 (right column, bottom)
//
//   Directly connected to 3.3V logic — no level conversion needed.
//   Buttons short to GND when pressed; internal pull-ups are enabled.
//
// === K-Line Interface (OBD-II diagnostics, ISO 9141/14230) ===
//
//   PB10 (USART3_TX) — K-Line TX (to transceiver)
//   PB11 (USART3_RX) — K-Line RX (from transceiver)
//
//   Signal chain: K-Line bus (12V) ↔ L9637D adapter (5V) ↔ BSS138 shifter (3.3V) ↔ STM32
//
//   L9637D adapter: converts 12V K-Line bus to 5V UART.
//   BSS138 4-ch bidirectional level shifter: converts 5V ↔ 3.3V.
//
//   BSS138 board wiring:
//     HV  → 5V  (from L9637D adapter VCC)
//     LV  → 3.3V (from STM32 3.3V rail)
//     GND → common ground
//     HV1 → L9637D TX pin (5V UART out to K-Line bus)
//     LV1 → STM32 PB10 (USART3_TX)
//     HV2 → L9637D RX pin (5V UART in from K-Line bus)
//     LV2 → STM32 PB11 (USART3_RX)
//
//   USART3 configured at 10400 baud (ISO 9141-2 / KWP2000 slow init).
//
// === USB (data link to Raspberry Pi) ===
//
//   PA11 (USB_DM) — USB D-
//   PA12 (USB_DP) — USB D+
//
//   Native USB on STM32F103. Used for the main serial data stream.
//
// ============================================================
// Pin Conflict Resolution
// ============================================================
//
//   PA11 is shared between USART1_TX and USB_DM.
//   Resolution: K-Line uses USART3 (PB10/PB11) instead of USART1.
//
//   PB10 was originally Diff lock indicator.
//   Resolution: Diff lock moved to PA15.
//
// ============================================================
// Final Pin Assignment Summary
// ============================================================
//
//   Pin   | Function                | Type        | Notes
//   ------|-------------------------|-------------|---------------------------
//   PA0   | Oil pressure (analog)   | ADC_IN0     | Voltage divider from sensor
//   PA1   | Fuel level (analog)     | ADC_IN1     | Voltage divider from sensor
//   PA2   | Coolant temp (analog)   | ADC_IN2     | Voltage divider from sensor
//   PA3   | 12V voltage (analog)    | ADC_IN3     | Resistive divider 20V→3.3V
//   PA4   | Button 4                | GPIO IN PU  | Active-low, 3.3V direct
//   PA5   | Button 5                | GPIO IN PU  | Active-low, 3.3V direct
//   PA6   | Button 6                | GPIO IN PU  | Active-low, 3.3V direct
//   PA7   | Button 7                | GPIO IN PU  | Active-low, 3.3V direct
//   PA8   | Oil pressure warning    | GPIO IN PU  | Active-low, level shifted
//   PA9   | Fuel low warning        | GPIO IN PU  | Active-low, level shifted
//   PA10  | (free / future use)     |             |
//   PA11  | USB D-                  | USB         | To Raspberry Pi
//   PA12  | USB D+                  | USB         | To Raspberry Pi
//   PA15  | Diff lock indicator     | GPIO IN PU  | Active-low, level shifted
//   PB0   | Tachometer pulse        | EXTI0       | Divider + 1nF cap, level shifted
//   PB1   | Speed sensor pulse      | EXTI1       | Divider + 1nF cap, level shifted
//   PB3   | Charging indicator      | GPIO IN PU  | Active-low, level shifted
//   PB4   | Exterior lights         | GPIO IN     | Active-high, level shifted
//   PB5   | Brake fluid low         | GPIO IN     | Active-high, level shifted
//   PB6   | Headlights on           | GPIO IN     | Active-high, level shifted
//   PB7   | Turn signal on          | GPIO IN     | Active-high, level shifted
//   PB8   | High beams on           | GPIO IN     | Active-high, level shifted
//   PB9   | Parking brake on        | GPIO IN PU  | Active-low, level shifted
//   PB10  | K-Line TX (USART3_TX)   | UART TX     | Via L9637D + BSS138 shifter
//   PB11  | K-Line RX (USART3_RX)   | UART RX     | Via L9637D + BSS138 shifter
//   PB12  | Button 0                | GPIO IN PU  | Active-low, 3.3V direct
//   PB13  | Button 1                | GPIO IN PU  | Active-low, 3.3V direct
//   PB14  | Button 2                | GPIO IN PU  | Active-low, 3.3V direct
//   PB15  | Button 3                | GPIO IN PU  | Active-low, 3.3V direct
//
//   Reserved/used by system:
//   PA13  | SWDIO                   | Debug       | SWD programming
//   PA14  | SWCLK                   | Debug       | SWD programming
//   PB2   | BOOT1                   | Boot config | Tie to GND for normal boot
//   PC13  | On-board LED            | GPIO OUT    | Heartbeat / status blink
//   PC14  | OSC32_IN                | RTC crystal | (if 32kHz crystal fitted)
//   PC15  | OSC32_OUT               | RTC crystal | (if 32kHz crystal fitted)
//
// ============================================================
// Free pins (available for future expansion):
//   PA10, PB2 (if BOOT1 not needed at runtime)
// ============================================================
//
// Problem with USB enumeration on Blue Pill clones: R10 resistor across 3v3
// and D+ (PA12) has wrong value (10kΩ instead of 1.5kΩ)
// Hardware fix applied on this board: 2kΩ resistor soldered in parallel
// with R10 (10kΩ), giving ~1.67kΩ effective — within USB Full Speed spec.

#include <Arduino.h>
#include <HardwareTimer.h>

// ============================================================
// Pin definitions
// ============================================================

// Analog inputs (12-bit ADC via voltage dividers)
#define PIN_OIL_PRESS_ANA   PA0
#define PIN_FUEL_LEVEL_ANA  PA1
#define PIN_COOLANT_ANA     PA2
#define PIN_VOLTAGE_ANA     PA3

// Pulse inputs (EXTI interrupt-based counting)
#define PIN_TACHO           PB0   // Tachometer, 2 PPR
#define PIN_SPEED           PB1   // Speed sensor, 4 PPR

// Digital indicators — active-low (INPUT_PULLUP)
#define PIN_D_OIL_WARN      PA8   // D0
#define PIN_D_FUEL_WARN     PA9   // D1
#define PIN_D_CHARGING      PB3   // D2
#define PIN_D_PARKING       PB9   // D8
#define PIN_D_DIFF_LOCK     PA15  // D9

// Digital indicators — active-high (INPUT, R2 acts as pull-down)
#define PIN_D_EXT_LIGHTS    PB4   // D3
#define PIN_D_BRAKE_FLUID   PB5   // D4
#define PIN_D_HEADLIGHTS    PB6   // D5
#define PIN_D_TURN          PB7   // D6
#define PIN_D_HIGHBEAMS     PB8   // D7

// Buttons — active-low (INPUT_PULLUP), B0..B7
static const uint32_t BTN_PINS[8] = {
    PB12, PB13, PB14, PB15,   // B0..B3 (left column)
    PA4,  PA5,  PA6,  PA7     // B4..B7 (right column)
};

// K-Line UART (USART3 via L9637D + BSS138)
// RX=PB11, TX=PB10
HardwareSerial KLine(PB11, PB10);

// Onboard LED (active-low)
#define PIN_LED             PC13

// ============================================================
// Configuration
// ============================================================

#define TICK_HZ             50          // data frame rate (Hz)
#define ADC_OVERSAMPLE      16          // samples averaged per ADC channel
#define BTN_DEBOUNCE_MASK   0xFF        // 8 consecutive reads to confirm state
#define KLINE_BUF_SIZE      64          // K-Line RX ring buffer size

// ============================================================
// Pulse counters — updated in ISR, read atomically in loop
// ============================================================

static volatile uint32_t tacho_count = 0;
static volatile uint32_t speed_count = 0;

void tacho_isr() { tacho_count++; }
void speed_isr() { speed_count++; }

// ============================================================
// 50 Hz tick flag — set by hardware timer ISR
// ============================================================

static volatile bool tick_flag = false;

void on_tick() { tick_flag = true; }

// ============================================================
// Button debounce state
// ============================================================

static uint8_t btn_history[8];
static uint8_t btn_state[8];

// ============================================================
// K-Line RX ring buffer
// ============================================================

static uint8_t kline_buf[KLINE_BUF_SIZE];
static uint8_t kline_head = 0;
static uint8_t kline_tail = 0;

// ============================================================
// Helpers
// ============================================================

// Average ADC_OVERSAMPLE reads — reduces noise, effective extra bits
static uint16_t read_adc_avg(uint32_t pin) {
    uint32_t sum = 0;
    for (int i = 0; i < ADC_OVERSAMPLE; i++) {
        sum += analogRead(pin);
    }
    return (uint16_t)(sum / ADC_OVERSAMPLE);
}

// Active-low: LOW = asserted = 1
static inline uint8_t read_lo(uint32_t pin) {
    return digitalRead(pin) == LOW ? 1 : 0;
}

// Active-high: HIGH = asserted = 1
static inline uint8_t read_hi(uint32_t pin) {
    return digitalRead(pin) == HIGH ? 1 : 0;
}

// ============================================================
// setup()
// ============================================================

void setup() {
    // LED on during init
    pinMode(PIN_LED, OUTPUT);
    digitalWrite(PIN_LED, LOW);

    // ADC: 12-bit resolution (default on STM32, explicit for clarity)
    analogReadResolution(12);

    // Pulse inputs — no pull (external divider + Zener provides defined levels)
    pinMode(PIN_TACHO, INPUT);
    pinMode(PIN_SPEED, INPUT);
    attachInterrupt(digitalPinToInterrupt(PIN_TACHO), tacho_isr, RISING);
    attachInterrupt(digitalPinToInterrupt(PIN_SPEED), speed_isr, RISING);

    // Digital indicators — active-low (external divider idles at ~3.3V = HIGH)
    pinMode(PIN_D_OIL_WARN,    INPUT_PULLUP);
    pinMode(PIN_D_FUEL_WARN,   INPUT_PULLUP);
    pinMode(PIN_D_CHARGING,    INPUT_PULLUP);
    pinMode(PIN_D_PARKING,     INPUT_PULLUP);
    pinMode(PIN_D_DIFF_LOCK,   INPUT_PULLUP);

    // Digital indicators — active-high (R2 pull-down holds 0V when signal is off)
    pinMode(PIN_D_EXT_LIGHTS,  INPUT);
    pinMode(PIN_D_BRAKE_FLUID, INPUT);
    pinMode(PIN_D_HEADLIGHTS,  INPUT);
    pinMode(PIN_D_TURN,        INPUT);
    pinMode(PIN_D_HIGHBEAMS,   INPUT);

    // Buttons — active-low, direct 3.3V connection
    for (int i = 0; i < 8; i++) {
        pinMode(BTN_PINS[i], INPUT_PULLUP);
        btn_history[i] = BTN_DEBOUNCE_MASK; // assume released at startup
        btn_state[i] = 0;
    }

    // K-Line UART — ISO 9141-2 / KWP2000 baud rate
    KLine.begin(10400);

    // Serial.begin() hands PA12 to the USB peripheral from here
    Serial.begin(115200);

    // 50 Hz tick timer — TIM2 (free on Blue Pill, not used by Arduino core)
    HardwareTimer *ticker = new HardwareTimer(TIM2);
    ticker->setOverflow(TICK_HZ, HERTZ_FORMAT);
    ticker->attachInterrupt(on_tick);
    ticker->resume();

    // Init complete — LED off
    digitalWrite(PIN_LED, HIGH);
}

// ============================================================
// loop()
// ============================================================

void loop() {
    // Spin until the 50 Hz tick fires
    if (!tick_flag) return;
    tick_flag = false;

    // ----------------------------------------------------------
    // 1. ADC — 4 channels, oversampled
    // ----------------------------------------------------------
    uint16_t adc[4];
    adc[0] = read_adc_avg(PIN_OIL_PRESS_ANA);
    adc[1] = read_adc_avg(PIN_FUEL_LEVEL_ANA);
    adc[2] = read_adc_avg(PIN_COOLANT_ANA);
    adc[3] = read_adc_avg(PIN_VOLTAGE_ANA);

    // ----------------------------------------------------------
    // 2. Pulse counters — atomic snapshot and reset
    // ----------------------------------------------------------
    noInterrupts();
    uint32_t tacho = tacho_count; tacho_count = 0;
    uint32_t speed = speed_count; speed_count = 0;
    interrupts();

    // ----------------------------------------------------------
    // 3. Digital indicators — D0..D9
    // ----------------------------------------------------------
    uint8_t d[10];
    d[0] = read_lo(PIN_D_OIL_WARN);     // PA8,  active-low
    d[1] = read_lo(PIN_D_FUEL_WARN);    // PA9,  active-low
    d[2] = read_lo(PIN_D_CHARGING);     // PB3,  active-low
    d[3] = read_hi(PIN_D_EXT_LIGHTS);   // PB4,  active-high
    d[4] = read_hi(PIN_D_BRAKE_FLUID);  // PB5,  active-high
    d[5] = read_hi(PIN_D_HEADLIGHTS);   // PB6,  active-high
    d[6] = read_hi(PIN_D_TURN);         // PB7,  active-high
    d[7] = read_hi(PIN_D_HIGHBEAMS);    // PB8,  active-high
    d[8] = read_lo(PIN_D_PARKING);      // PB9,  active-low
    d[9] = read_lo(PIN_D_DIFF_LOCK);    // PA15, active-low

    // ----------------------------------------------------------
    // 4. Buttons — shift-register debounce, B0..B7
    //    Active-low: 8 consecutive LOWs = pressed (history == 0x00)
    //                8 consecutive HIGHs = released (history == 0xFF)
    //                transitional: state unchanged
    // ----------------------------------------------------------
    for (int i = 0; i < 8; i++) {
        uint8_t bit = (digitalRead(BTN_PINS[i]) == HIGH) ? 1 : 0;
        btn_history[i] = (btn_history[i] << 1) | bit;
        if      (btn_history[i] == 0x00) btn_state[i] = 1; // confirmed pressed
        else if (btn_history[i] == 0xFF) btn_state[i] = 0; // confirmed released
        // else: bouncing — hold last known state
    }

    // ----------------------------------------------------------
    // 5. K-Line RX — drain USART3 into ring buffer each tick
    //    Full ISO 9141 state machine to be added as separate module
    // ----------------------------------------------------------
    while (KLine.available()) {
        uint8_t byte = (uint8_t)KLine.read();
        uint8_t next = (kline_head + 1) % KLINE_BUF_SIZE;
        if (next != kline_tail) {          // drop byte if buffer full
            kline_buf[kline_head] = byte;
            kline_head = next;
        }
    }

    // ----------------------------------------------------------
    // 6. Heartbeat LED — toggle every 25 ticks (0.5 s)
    // ----------------------------------------------------------
    static uint8_t led_tick = 0;
    if (++led_tick >= 25) {
        led_tick = 0;
        digitalWrite(PIN_LED, !digitalRead(PIN_LED));
    }

    // ----------------------------------------------------------
    // 7. Transmit data frame over USB to Raspberry Pi
    //    Format: $A0,A1,A2,A3,TACHO,SPEED,D0..D9,B0..B7\n
    // ----------------------------------------------------------
    char frame[128];
    snprintf(frame, sizeof(frame),
        "$%u,%u,%u,%u,%u,%u,"
        "%u,%u,%u,%u,%u,%u,%u,%u,%u,%u,"
        "%u,%u,%u,%u,%u,%u,%u,%u\n",
        adc[0], adc[1], adc[2], adc[3],
        (unsigned)tacho, (unsigned)speed,
        d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7], d[8], d[9],
        btn_state[0], btn_state[1], btn_state[2], btn_state[3],
        btn_state[4], btn_state[5], btn_state[6], btn_state[7]
    );
    Serial.print(frame);
}
