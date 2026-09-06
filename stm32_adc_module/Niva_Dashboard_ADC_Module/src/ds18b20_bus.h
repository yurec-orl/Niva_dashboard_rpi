// ============================================================
// One-Wire DS18B20 temperature bus — PA10
// ============================================================
//
// Autonomous DS18B20 acquisition for the Niva Dashboard ADC module.
// See ONEWIRE_TEMP_SENSOR_DESIGN.md for the wire protocol and rationale.
//
// The bus is discovered once at startup (SEARCH ROM), then polled forever by a
// non-blocking state machine ticked from the 50 Hz loop(). Each convert->read
// cycle emits one tagged line, interleaved with normal telemetry:
//
//   $T,<rom>:<raw>;<rom>:<raw>;...\n
//
// <rom> is the 8 ROM bytes as 16 lowercase hex chars; <raw> is the signed
// DS18B20 temperature register in 1/16 deg C units. The STM32 has no knowledge
// of what any sensor measures — the dashboard owns the address->sensor mapping.

#pragma once

// Initialise bus state and begin ROM discovery. Call once from setup().
void ds18b20_setup();

// Advance the bus state machine by one step (one 1-Wire transaction at most).
// Call once per 50 Hz tick from loop(). Never blocks on conversion latency.
void ds18b20_tick();
