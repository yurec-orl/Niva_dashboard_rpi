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
//   - TACHO:   average period between tachometer pulses since last report, in units of
//              TACHO_PERIOD_UNIT_US (tachometer, 2 PPR). 0 = no pulse for over
//              TACHO_TIMEOUT_US (stalled, or below the ~46 rpm floor this encoding can
//              represent). NOT a pulse count — see "Tachometer timing" below for why.
//   - SPEED:   average period between speed-sensor pulses since last report, in units of
//              SPEED_PERIOD_UNIT_US (speed sensor, 6 PPR). 0 = no pulse for over
//              SPEED_TIMEOUT_US (stopped, or below the ~2 km/h floor this encoding can
//              represent). NOT a pulse count — see "Speed sensor timing" below for why.
//   - D0..D9:  digital indicator states (0/1)
//   - B0..B7:  button states (0/1, 1 = pressed)
//
// Oscilloscope burst-capture command/response (see OSCILLOSCOPE_STM32_IMPLEMENTATION_PLAN.md):
//   Pi -> STM32: "$OSCCAP\n"  — request a one-shot high-rate capture on PA3 (12V bus).
//   STM32 -> Pi: normal telemetry pauses; "$OSCD,<seq>,<v0>,<v1>,...\n" chunks stream the
//                captured buffer, followed by a "$OSCEND\n" sentinel; telemetry then resumes.
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
//   PB1  (EXTI1) — Speed sensor signal, 6 pulses per revolution (previously mistaken for
//                  a 4 PPR sensor — see "Speed sensor timing" below for the consequence
//                  this had on the reported value, independent of the PPR mixup itself)
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
//   PB14 — Button 0 (left column, top)
//   PB15 — Button 1 (left column, 2nd)
//   PB13 — Button 2 (left column, 3rd)
//   PB12 — Button 3 (left column, bottom)
//   PA5  — Button 4 (right column, top)
//   PA4  — Button 5 (right column, 2nd)
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
//   PA3   | 12V voltage (analog)    | ADC_IN3     | Resistive divider 20V→3.3V. Time-shared:
//         |                         |             | normal oversampled telemetry reads vs.
//         |                         |             | "$OSCCAP" burst-capture DMA streaming —
//         |                         |             | mutually exclusive, capture is blocking.
//   PA4   | Button 5                | GPIO IN PU  | Active-low, 3.3V direct
//   PA5   | Button 4                | GPIO IN PU  | Active-low, 3.3V direct
//   PA6   | Button 6                | GPIO IN PU  | Active-low, 3.3V direct
//   PA7   | Button 7                | GPIO IN PU  | Active-low, 3.3V direct
//   PA8   | Oil pressure warning    | GPIO IN PU  | Active-low, level shifted
//   PA9   | Fuel low warning        | GPIO IN PU  | Active-low, level shifted
//   PA10  | (reserved: DS18B20)     | 1-Wire      | Planned one-wire temp sensor bus, not yet implemented
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
//   PB12  | Button 3                | GPIO IN PU  | Active-low, 3.3V direct
//   PB13  | Button 2                | GPIO IN PU  | Active-low, 3.3V direct
//   PB14  | Button 0                | GPIO IN PU  | Active-low, 3.3V direct
//   PB15  | Button 1                | GPIO IN PU  | Active-low, 3.3V direct
//
//   Reserved/used by system:
//   PA13  | SWDIO                   | Debug       | SWD programming
//   PA14  | SWCLK                   | Debug       | SWD programming
//   PB2   | BOOT1                   | Boot config | Tie to GND for normal boot
//   PC13  | On-board LED            | GPIO OUT    | Heartbeat / status blink
//   PC14  | OSC32_IN                | RTC crystal | (if 32kHz crystal fitted)
//   PC15  | OSC32_OUT               | RTC crystal | (if 32kHz crystal fitted)
//
//   Timer/DMA resource claims (no pins involved):
//   TIM3          | Oscilloscope ADC trigger — TRGO on update, 50 kHz, free-running only
//                 | during a "$OSCCAP" capture. No NVIC interrupt attached.
//   DMA1 Channel1 | ADC1's fixed (non-remappable on F103) DMA channel — used only during
//                 | a "$OSCCAP" capture to stream PA3 samples into osc_buffer.
//
// ============================================================
// Free pins (available for future expansion):
//   PA10 — reserved for a planned DS18B20 one-wire temp sensor bus (not yet implemented)
//   PB2  — free if BOOT1 not needed at runtime (currently tied to GND for normal boot,
//          so using it as GPIO would require removing that strap)
// ============================================================
//
// Problem with USB enumeration on Blue Pill clones: R10 resistor across 3v3
// and D+ (PA12) has wrong value (10kΩ instead of 1.5kΩ)
// Hardware fix applied on this board: 2kΩ resistor soldered in parallel
// with R10 (10kΩ), giving ~1.67kΩ effective — within USB Full Speed spec.
//
// ============================================================
// Build & Upload
// ============================================================
//
//   Toolchain: PlatformIO (platform=ststm32, framework=arduino/STM32duino,
//   board=genericSTM32F103C8). See platformio.ini.
//
//   Upload: upload_protocol = stlink — flashed via an ST-Link programmer
//   wired to SWDIO/SWCLK (PA13/PA14), not USB DFU or serial bootloader.
//
//   IMPORTANT: open this project from the PlatformIO Home UI (not by
//   plain "Open Folder" in VS Code) — otherwise the PlatformIO extension
//   may not pick up the project environment correctly and build/upload
//   will fail.

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
#define PIN_SPEED           PB1   // Speed sensor, 6 PPR

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
    PB14, PB15, PB13, PB12,   // B0..B3 (left column, top to bottom)
    PA5,  PA4,  PA6,  PA7     // B4..B7 (right column, top to bottom)
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

// ------------------------------------------------------------
// Oscilloscope burst capture (see OSCILLOSCOPE_STM32_IMPLEMENTATION_PLAN.md)
// ------------------------------------------------------------
// One-shot high-rate capture on PA3 (12V bus), triggered by the Pi sending "$OSCCAP".
// Normal 50 Hz telemetry is blocked for the ~82 ms capture duration (synchronous/blocking
// by design — this is a manual, occasional diagnostic action, not a continuous stream).
#define OSC_ADC_CHANNEL      ADC_CHANNEL_3   // PA3, same pin as telemetry's 12V channel
#define OSC_SAMPLE_RATE_HZ   50000UL         // TIM3 TRGO rate driving the ADC
#define OSC_BUF_LEN          4096            // samples per capture (~82 ms window)
#define OSC_CHUNK_SAMPLES    64              // samples per "$OSCD,<seq>,..." line
#define OSC_DMA_TIMEOUT_MS   150UL           // bounds the capture; expected ~82 ms

// ------------------------------------------------------------
// Speed sensor timing
// ------------------------------------------------------------
// At TICK_HZ=50 (20 ms/frame) a 6 PPR sensor does not produce an integer pulse count per
// frame across the realistic speed range (up to 150 km/h ~= 108 Hz ~= one pulse every
// 9.2 ms): below ~69 km/h most frames see 0 or 1 new pulses, and which frame a given pulse
// lands in shifts over time relative to the 20 ms grid. Reporting a per-frame pulse *count*
// therefore aliases into a jittery reading even at constant speed. Timing the gap between
// pulses (in the ISR, independent of the frame boundary) and reporting that period instead
// removes the aliasing: the period is a direct, frame-rate-independent measurement of
// instantaneous speed.
//
// SPEED_PERIOD_UNIT_US sets the wire encoding's resolution/range trade-off in a 16-bit
// field: at 10 us/unit, periods up to 65535*10us = 655.35 ms fit (down to a ~2.1 km/h
// floor), while resolving ~0.16 km/h steps at the 150 km/h / 9.2 ms end. Below the floor
// (or when stopped), SPEED_TIMEOUT_US of silence makes the frame report 0 rather than
// hanging onto a stale reading.
#define SPEED_PERIOD_UNIT_US 10UL        // wire units for the SPEED field (10 us/unit)
#define SPEED_TIMEOUT_US     1000000UL   // no pulse for this long => report stopped (0)

// ------------------------------------------------------------
// Tachometer timing
// ------------------------------------------------------------
// Same aliasing problem as speed (see "Speed sensor timing" above), but with a worse
// symptom on the RPi side today: TACHO currently feeds a boolean "engine running" debounce
// requiring 3 consecutive 20 ms frames to all see a pulse. At 2 PPR that requirement is
// mathematically unsatisfiable below 1000 rpm (pulse period >= 30 ms) and phase-dependent/
// flaky up to 1500 rpm — i.e. it can never latch true anywhere in the normal idle range
// (400-800 rpm). Sending period instead of count fixes this the same way it fixes SPEED:
// a period reading doesn't depend on how many pulses landed inside one particular 20 ms
// frame, so idle (period ~37.5 ms at 800 rpm, updating a little less than every other
// frame) is measured correctly instead of never confirming at all.
//
// Reuses SPEED's 10 us/unit encoding — it comfortably covers the full engine range: down
// to a ~46 rpm floor (65535 * 10us = 655.35 ms) and resolving ~12 rpm steps at a 6000 rpm
// redline. TACHO_TIMEOUT_US is shorter than SPEED_TIMEOUT_US since engine rpm changes (and
// stalls) faster than road speed does.
#define TACHO_PERIOD_UNIT_US 10UL        // wire units for the TACHO field (10 us/unit)
#define TACHO_TIMEOUT_US     500000UL    // no pulse for this long => report stalled (0)

// ============================================================
// Pulse counters — updated in ISR, read atomically in loop
// ============================================================

// Tachometer pulse *timing* — same scheme as speed_isr below. See "Tachometer timing"
// above for why period, not count, is measured.
static volatile uint32_t tacho_last_edge_us = 0;
static volatile uint32_t tacho_period_sum_us = 0;
static volatile uint16_t tacho_period_count = 0;
static volatile bool     tacho_edge_seen = false;

void tacho_isr() {
    uint32_t now = micros();
    if (tacho_edge_seen) {
        // Unsigned subtraction wraps correctly even across a micros() rollover (~71.6 min).
        tacho_period_sum_us += (now - tacho_last_edge_us);
        tacho_period_count++;
    }
    tacho_last_edge_us = now;
    tacho_edge_seen = true;
}

// Speed pulse *timing* — accumulates completed inter-pulse periods (for averaging when
// more than one pulse lands in a frame) instead of counting pulses. See "Speed sensor
// timing" above.
static volatile uint32_t speed_last_edge_us = 0;
static volatile uint32_t speed_period_sum_us = 0;
static volatile uint16_t speed_period_count = 0;
static volatile bool     speed_edge_seen = false;

void speed_isr() {
    uint32_t now = micros();
    if (speed_edge_seen) {
        // Unsigned subtraction wraps correctly even across a micros() rollover (~71.6 min).
        speed_period_sum_us += (now - speed_last_edge_us);
        speed_period_count++;
    }
    speed_last_edge_us = now;
    speed_edge_seen = true;
}

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
// Oscilloscope capture buffer and incoming-command line buffer
// ============================================================

// File-scope static, not stack: ~8 KB would blow loop()'s stack frame.
static uint16_t osc_buffer[OSC_BUF_LEN];

// Lazily-constructed TIM3 handle, reused across captures. TIM3 only drives the ADC's
// hardware trigger (TRGO on update) here — no NVIC interrupt is attached to it.
static HardwareTimer *osc_trigger_timer = nullptr;

// Shared inbound-serial line reader — parses "$OSCCAP" today, and is the landing spot
// for the brightness command ("#B,<value>") proposed in BUTTON_BACKLIGHT_DESIGN.md.
static char cmd_line[32];
static uint8_t cmd_len = 0;

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
// Oscilloscope capture — "$OSCCAP" command handling
// ============================================================

// Streams osc_buffer back as chunked "$OSCD,<seq>,<v0>,<v1>,...\n" lines followed by a
// "$OSCEND\n" sentinel. Uses its own frame buffer rather than the 128-byte telemetry
// `frame[]` in loop() — that one is on the hot 50 Hz path and shouldn't grow to fit a
// rare, large, one-shot transfer.
static void oscilloscope_send_buffer() {
    char osc_frame[OSC_CHUNK_SAMPLES * 5 + 16]; // "$OSCD,<seq>" + up to 64x",4095" + "\n"
    uint16_t seq = 0;
    for (uint16_t i = 0; i < OSC_BUF_LEN; i += OSC_CHUNK_SAMPLES, seq++) {
        int n = snprintf(osc_frame, sizeof(osc_frame), "$OSCD,%u", (unsigned)seq);
        for (uint16_t j = 0; j < OSC_CHUNK_SAMPLES; j++) {
            n += snprintf(osc_frame + n, sizeof(osc_frame) - n, ",%u", osc_buffer[i + j]);
        }
        snprintf(osc_frame + n, sizeof(osc_frame) - n, "\n");
        Serial.print(osc_frame);
    }
    Serial.print("$OSCEND\n");
}

// Blocking one-shot capture: pauses normal telemetry (implicitly — this runs synchronously
// from the command dispatcher, called before loop()'s tick-flag check), captures OSC_BUF_LEN
// samples on PA3 at OSC_SAMPLE_RATE_HZ via TIM3-triggered ADC1 DMA, then restores ADC1 to
// the state analogRead() expects and sends the buffer back.
//
// analogRead() (stm32duino core) already performs a full HAL_ADC_Init/.../HAL_ADC_DeInit
// cycle on every single call — it never assumes any particular prior ADC1 state. That means
// this function only needs to leave ADC1 in a de-initialized state on exit; it does not need
// to reconstruct analogRead()'s specific configuration.
static void run_oscilloscope_capture() {
    // PA3 may not be in analog mode if this is called unusually early (before the first
    // telemetry tick has run analogRead() on it) — force it explicitly rather than relying
    // on that ordering.
    pinMode(PIN_VOLTAGE_ANA, INPUT_ANALOG);

    // --- TIM3: free-running trigger source, TRGO on update, no NVIC interrupt ---
    if (osc_trigger_timer == nullptr) {
        osc_trigger_timer = new HardwareTimer(TIM3);
    }
    osc_trigger_timer->pause();
    osc_trigger_timer->setOverflow(OSC_SAMPLE_RATE_HZ, HERTZ_FORMAT);
    TIM_MasterConfigTypeDef sMasterConfig = {};
    sMasterConfig.MasterOutputTrigger = TIM_TRGO_UPDATE;
    sMasterConfig.MasterSlaveMode = TIM_MASTERSLAVEMODE_DISABLE;
    HAL_TIMEx_MasterConfigSynchronization(osc_trigger_timer->getHandle(), &sMasterConfig);

    // --- DMA1 Channel1: ADC1's fixed (non-remappable) DMA channel on F103 ---
    static DMA_HandleTypeDef hdma_osc;
    hdma_osc = DMA_HandleTypeDef{};
    hdma_osc.Instance = DMA1_Channel1;
    hdma_osc.Init.Direction = DMA_PERIPH_TO_MEMORY;
    hdma_osc.Init.PeriphInc = DMA_PINC_DISABLE;
    hdma_osc.Init.MemInc = DMA_MINC_ENABLE;
    hdma_osc.Init.PeriphDataAlignment = DMA_PDATAALIGN_HALFWORD;
    hdma_osc.Init.MemDataAlignment = DMA_MDATAALIGN_HALFWORD;
    hdma_osc.Init.Mode = DMA_NORMAL; // one-shot, not circular — matches a single capture
    hdma_osc.Init.Priority = DMA_PRIORITY_HIGH;
    HAL_DMA_DeInit(&hdma_osc);
    HAL_DMA_Init(&hdma_osc);

    // --- ADC1: single channel, hardware-triggered by TIM3 TRGO, DMA destination ---
    static ADC_HandleTypeDef hadc_osc;
    hadc_osc = ADC_HandleTypeDef{};
    hadc_osc.Instance = ADC1;
    hadc_osc.Init.DataAlign = ADC_DATAALIGN_RIGHT;
    hadc_osc.Init.ScanConvMode = DISABLE;
    hadc_osc.Init.ContinuousConvMode = DISABLE; // one conversion per TRGO, not free-run
    hadc_osc.Init.NbrOfConversion = 1;
    hadc_osc.Init.DiscontinuousConvMode = DISABLE;
    hadc_osc.Init.NbrOfDiscConversion = 0;
    hadc_osc.Init.ExternalTrigConv = ADC_EXTERNALTRIGCONV_T3_TRGO;
    __HAL_LINKDMA(&hadc_osc, DMA_Handle, hdma_osc);
    HAL_ADC_DeInit(&hadc_osc);
    HAL_ADC_Init(&hadc_osc);

    ADC_ChannelConfTypeDef sConfig = {};
    sConfig.Channel = OSC_ADC_CHANNEL;
    sConfig.Rank = ADC_REGULAR_RANK_1;
    sConfig.SamplingTime = ADC_SAMPLETIME_55CYCLES_5; // ~5.7us conv time, ample margin at 20us/sample
    HAL_ADC_ConfigChannel(&hadc_osc, &sConfig);
    HAL_ADCEx_Calibration_Start(&hadc_osc);

    // Arm DMA+ADC first (idle, waiting for TRGO), then start the timer so the first sample
    // lands cleanly on the first trigger instead of racing timer startup.
    HAL_ADC_Start_DMA(&hadc_osc, (uint32_t *)osc_buffer, OSC_BUF_LEN);
    osc_trigger_timer->resume();

    HAL_DMA_PollForTransfer(&hdma_osc, HAL_DMA_FULL_TRANSFER, OSC_DMA_TIMEOUT_MS);

    osc_trigger_timer->pause();
    HAL_ADC_Stop_DMA(&hadc_osc);
    HAL_ADC_DeInit(&hadc_osc);
    HAL_DMA_DeInit(&hdma_osc);

    oscilloscope_send_buffer();
}

static void dispatch_command(const char *line) {
    if (strcmp(line, "$OSCCAP") == 0) {
        run_oscilloscope_capture();
    }
    // else if (strncmp(line, "#B,", 3) == 0) { ... brightness, per BUTTON_BACKLIGHT_DESIGN.md ... }
}

// Non-blocking; drains whatever is available and dispatches complete lines. Cheap when
// Serial is idle. Called once per loop() iteration, ahead of the tick-flag check, so a
// command can be received and its (blocking) handler run even on a tick that doesn't fire.
static void poll_incoming_commands() {
    while (Serial.available()) {
        char c = (char)Serial.read();
        if (c == '\n') {
            cmd_line[cmd_len] = '\0';
            dispatch_command(cmd_line);
            cmd_len = 0;
        } else if (cmd_len < sizeof(cmd_line) - 1) {
            cmd_line[cmd_len++] = c;
        } else {
            cmd_len = 0; // overlong line — drop and resync on next '\n'
        }
    }
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
    // Drain/dispatch any inbound command line (e.g. "$OSCCAP") every pass, independent of
    // the tick — a capture command's (blocking) handler runs here, ahead of the tick check.
    poll_incoming_commands();

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
    uint32_t tacho_period_sum = tacho_period_sum_us; tacho_period_sum_us = 0;
    uint16_t tacho_period_cnt = tacho_period_count; tacho_period_count = 0;
    uint32_t tacho_last_edge = tacho_last_edge_us;
    bool tacho_has_edge = tacho_edge_seen;
    uint32_t speed_period_sum = speed_period_sum_us; speed_period_sum_us = 0;
    uint16_t speed_period_cnt = speed_period_count; speed_period_count = 0;
    uint32_t speed_last_edge = speed_last_edge_us;
    bool speed_has_edge = speed_edge_seen;
    interrupts();

    // Tacho field: same period-averaging scheme as SPEED below (see "Tachometer timing"
    // above). At the 6000 rpm ceiling (~5 ms/pulse) at most 4 periods complete per 20 ms
    // frame, so a plain average is enough — no weighting needed.
    static uint16_t tacho_period_last = 0; // last reported TACHO field value, held across
                                            // frames with no new pulse (see below)
    uint16_t tacho_field;
    if (tacho_period_cnt > 0) {
        uint32_t avg_us = tacho_period_sum / tacho_period_cnt;
        uint32_t units = avg_us / TACHO_PERIOD_UNIT_US;
        tacho_period_last = (units > 0xFFFF) ? 0xFFFF : (uint16_t)units;
        tacho_field = tacho_period_last;
    } else if (tacho_has_edge && (micros() - tacho_last_edge) < TACHO_TIMEOUT_US) {
        // No pulse completed within this specific frame, but the gap since the last edge
        // is still a plausible sample of the current (slow) rpm — hold the last computed
        // period instead of dropping to 0 and back every other frame.
        tacho_field = tacho_period_last;
    } else {
        // No pulse for over TACHO_TIMEOUT_US: stalled, or below the encoding's floor.
        tacho_period_last = 0;
        tacho_field = 0;
    }

    // Speed field: average of whatever inter-pulse periods completed this frame (see
    // "Speed sensor timing" above for why period, not count, is sent). At the realistic
    // top speed (~150 km/h, ~9.2 ms/pulse) at most 2 periods complete per 20 ms frame, so a
    // plain average is enough — no weighting needed.
    static uint16_t speed_period_last = 0; // last reported SPEED field value, held across
                                            // frames with no new pulse (see below)
    uint16_t speed_field;
    if (speed_period_cnt > 0) {
        uint32_t avg_us = speed_period_sum / speed_period_cnt;
        uint32_t units = avg_us / SPEED_PERIOD_UNIT_US;
        speed_period_last = (units > 0xFFFF) ? 0xFFFF : (uint16_t)units;
        speed_field = speed_period_last;
    } else if (speed_has_edge && (micros() - speed_last_edge) < SPEED_TIMEOUT_US) {
        // No pulse completed within this specific frame, but the gap since the last edge
        // is still a plausible sample of the current (slow) speed — hold the last computed
        // period instead of dropping to 0 and back every other frame.
        speed_field = speed_period_last;
    } else {
        // No pulse for over SPEED_TIMEOUT_US: stopped, or below the encoding's floor.
        speed_period_last = 0;
        speed_field = 0;
    }

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
    //    TACHO and SPEED are both average inter-pulse periods (see "Tachometer timing" /
    //    "Speed sensor timing" above), not pulse counts.
    // ----------------------------------------------------------
    char frame[128];
    snprintf(frame, sizeof(frame),
        "$%u,%u,%u,%u,%u,%u,"
        "%u,%u,%u,%u,%u,%u,%u,%u,%u,%u,"
        "%u,%u,%u,%u,%u,%u,%u,%u\n",
        adc[0], adc[1], adc[2], adc[3],
        (unsigned)tacho_field, (unsigned)speed_field,
        d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7], d[8], d[9],
        btn_state[0], btn_state[1], btn_state[2], btn_state[3],
        btn_state[4], btn_state[5], btn_state[6], btn_state[7]
    );
    Serial.print(frame);
}
