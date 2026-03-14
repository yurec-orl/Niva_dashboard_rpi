// Niva Dashboard — Arduino Mega ADC/Sensor Module
// Reads analog sensors, digital indicators, and pulse signals (tacho/speed)
// Sends data over USB-serial to Raspberry Pi at 50 Hz
//
// Protocol: "$A0,A1,A2,A3,TACHO,SPEED,D47,D45,D43,D46,D44,D42,D40,D38,D36,D34,B25,B23,B27,B29,B24,B22,B26,B28\n"
// - A0..A3: raw 10-bit analog values (0-1023)
// - TACHO:  pulse count since last report (tachometer, 2 PPR)
// - SPEED:  pulse count since last report (speed sensor, 4 PPR)
// - D*:     digital pin states (0 or 1, after INPUT_PULLUP inversion where applicable)

#include <Arduino.h>

// ============================================================
// Pin definitions
// ============================================================

// Analog inputs
const uint8_t PIN_OIL_PRESSURE_AN  = A0;
const uint8_t PIN_FUEL_LEVEL       = A1;
const uint8_t PIN_COOLANT_TEMP     = A2;
const uint8_t PIN_VOLTAGE_12V      = A3;

const uint8_t ANALOG_PINS[] = {
  PIN_OIL_PRESSURE_AN,
  PIN_FUEL_LEVEL,
  PIN_COOLANT_TEMP,
  PIN_VOLTAGE_12V
};
const uint8_t ANALOG_PIN_COUNT = sizeof(ANALOG_PINS) / sizeof(ANALOG_PINS[0]);

// Interrupt-driven pulse inputs
const uint8_t PIN_TACHOMETER  = 18;  // INT3, 2 pulses per crankshaft revolution
const uint8_t PIN_SPEED       = 19;  // INT2, 4 pulses per wheel revolution

// Digital indicator inputs (directly read each cycle)
struct DigitalPin {
  uint8_t pin;
  uint8_t mode;       // INPUT or INPUT_PULLUP
  bool    invertRead; // true = active-low (INPUT_PULLUP sensors that short to GND)
};

const DigitalPin DIGITAL_PINS[] = {
  { 47, INPUT_PULLUP, true  },  // Oil pressure low warning
  { 45, INPUT_PULLUP, true  },  // Fuel low warning
  { 43, INPUT_PULLUP, true  },  // Charging indicator
  { 46, INPUT,        false },  // Exterior lights on
  { 44, INPUT,        false },  // Brake fluid low
  { 42, INPUT,        false },  // Headlights on
  { 40, INPUT,        false },  // Turn signal on
  { 38, INPUT,        false },  // High beams on
  { 36, INPUT_PULLUP, true  },  // Parking brake on
  { 34, INPUT_PULLUP, true  },  // Diff lock on
};
const uint8_t DIGITAL_PIN_COUNT = sizeof(DIGITAL_PINS) / sizeof(DIGITAL_PINS[0]);

// Physical dashboard buttons (INPUT_PULLUP, active low)
const uint8_t BUTTON_PINS[] = { 25, 23, 27, 29, 24, 22, 26, 28 };
const uint8_t BUTTON_PIN_COUNT = sizeof(BUTTON_PINS) / sizeof(BUTTON_PINS[0]);

// ============================================================
// Timing
// ============================================================

const unsigned long REPORT_INTERVAL_MS = 20;  // 50 Hz reporting rate

// ============================================================
// Serial
// ============================================================

const unsigned long SERIAL_BAUD = 115200;

// ============================================================
// Volatile counters incremented by ISRs
// ============================================================

volatile unsigned long tachoCount = 0;
volatile unsigned long speedCount = 0;

// ============================================================
// ISRs — keep as short as possible
// ============================================================

void tachometerISR() {
  tachoCount++;
}

void speedSensorISR() {
  speedCount++;
}

// ============================================================
// Setup
// ============================================================

void setup() {
  Serial.begin(SERIAL_BAUD);

  // Configure analog pins (default is input, but be explicit)
  for (uint8_t i = 0; i < ANALOG_PIN_COUNT; i++) {
    pinMode(ANALOG_PINS[i], INPUT);
  }

  // Configure digital indicator pins
  for (uint8_t i = 0; i < DIGITAL_PIN_COUNT; i++) {
    pinMode(DIGITAL_PINS[i].pin, DIGITAL_PINS[i].mode);
  }

  // Configure button pins
  for (uint8_t i = 0; i < BUTTON_PIN_COUNT; i++) {
    pinMode(BUTTON_PINS[i], INPUT_PULLUP);
  }

  // Configure interrupt pins and attach ISRs on rising edge
  pinMode(PIN_TACHOMETER, INPUT);
  pinMode(PIN_SPEED, INPUT);
  attachInterrupt(digitalPinToInterrupt(PIN_TACHOMETER), tachometerISR, RISING);
  attachInterrupt(digitalPinToInterrupt(PIN_SPEED), speedSensorISR, RISING);
}

// ============================================================
// Main loop — runs at REPORT_INTERVAL_MS cadence
// ============================================================

void loop() {
  static unsigned long lastReportTime = 0;
  unsigned long now = millis();

  if (now - lastReportTime < REPORT_INTERVAL_MS) {
    return;
  }
  lastReportTime = now;

  // --- Snapshot and reset pulse counters atomically ---
  noInterrupts();
  unsigned long tacho = tachoCount;
  unsigned long speed = speedCount;
  tachoCount = 0;
  speedCount = 0;
  interrupts();

  // --- Read analog values ---
  int analogValues[ANALOG_PIN_COUNT];
  for (uint8_t i = 0; i < ANALOG_PIN_COUNT; i++) {
    analogValues[i] = analogRead(ANALOG_PINS[i]);
  }

  // --- Read digital pin states ---
  uint8_t digitalValues[DIGITAL_PIN_COUNT];
  for (uint8_t i = 0; i < DIGITAL_PIN_COUNT; i++) {
    uint8_t raw = digitalRead(DIGITAL_PINS[i].pin);
    digitalValues[i] = DIGITAL_PINS[i].invertRead ? !raw : raw;
  }

  // --- Build and send message ---
  // Format: $TIMESTAMP,A0,A1,A2,A3,TACHO,SPEED,D0,D1,...,D9\n
  Serial.print('$');

  //Serial.print(now);  // Include timestamp for easier debugging and synchronization
  //Serial.print(',');

  for (uint8_t i = 0; i < ANALOG_PIN_COUNT; i++) {
    Serial.print(analogValues[i]);
    Serial.print(',');
  }

  Serial.print(tacho);
  Serial.print(',');
  Serial.print(speed);

  for (uint8_t i = 0; i < DIGITAL_PIN_COUNT; i++) {
    Serial.print(',');
    Serial.print(digitalValues[i]);
  }

  // Buttons (active low — invert so 1 = pressed)
  for (uint8_t i = 0; i < BUTTON_PIN_COUNT; i++) {
    Serial.print(',');
    Serial.print(!digitalRead(BUTTON_PINS[i]));
  }

  Serial.println();
}
