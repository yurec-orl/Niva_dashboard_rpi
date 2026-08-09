//! Concrete first use case for the fused analog sensor chain type (see
//! SENSOR_FUSION_CHAIN_DESIGN.md): a heading reading that prefers the BNO085 IMU (higher
//! update rate, works while stationary) and falls back to GNSS dual-antenna heading (needs
//! an RTK heading fix and doesn't update while stationary, but is drift-free) when the IMU
//! is absent or stale.

use crate::hardware::hw_providers::{Bno085ChannelProvider, GnssChannelProvider, HWInput, GNSS_HEADING_SCALE};
use crate::hardware::sensor_manager::SensorFusedAnalogInputChain;
use crate::hardware::sensor_value::{SensorValue, ValueConstraints, ValueData, ValueMetadata};
use crate::hardware::sensors::{FusedAnalogSensor, Sensor};
use crate::util::bno085_data_provider::Bno085Frame;
use crate::util::gnss_data_provider::GnssFrame;

/// Index into the `inputs` slice FusedAnalogSensor::read receives -- fixed by the provider
/// order new_heading_fusion_chain constructs the chain with, below. BNO085 is checked first
/// since it's the preferred source.
const BNO085_INDEX: usize = 0;
const GNSS_INDEX: usize = 1;

/// Fuses HwBno085Heading and HwGnssHeading into one HwHeading value. Both providers encode
/// heading with the same GNSS_HEADING_SCALE fixed-point scheme (see hw_providers.rs), so
/// they're decoded identically here regardless of which source actually won.
pub struct HeadingFusionSensor {
    value: SensorValue,
    constraints: ValueConstraints,
    metadata: ValueMetadata,
    last_heading_deg: Option<f32>,
}

impl HeadingFusionSensor {
    pub fn new() -> Self {
        HeadingFusionSensor {
            value: SensorValue::empty(),
            constraints: ValueConstraints::analog(0.0, 359.9),
            metadata: ValueMetadata::new("\u{00b0}", "КУРС", "heading_fused"),
            last_heading_deg: None,
        }
    }
}

impl Sensor for HeadingFusionSensor {
    fn id(&self) -> &String { &self.metadata.sensor_id }
    fn name(&self) -> &String { &self.metadata.label }
    fn value(&self) -> Result<&SensorValue, String> { Ok(&self.value) }
    fn constraints(&self) -> &ValueConstraints { &self.constraints }
    fn metadata(&self) -> &ValueMetadata { &self.metadata }
    fn min_value(&self) -> f32 { self.constraints.min_value }
    fn max_value(&self) -> f32 { self.constraints.max_value }
}

impl FusedAnalogSensor for HeadingFusionSensor {
    fn read(&mut self, inputs: &[Option<u16>]) -> Result<&SensorValue, String> {
        // BNO085 present -> trust it; BNO085 absent/stale, GNSS present -> fall back to GNSS;
        // neither -> ValueData::Empty (see SENSOR_FUSION_CHAIN_DESIGN.md's "Why Option<u16>
        // per provider" section -- this is exactly the graceful-degradation case a `?`
        // short-circuit can't express).
        let raw = inputs.get(BNO085_INDEX).copied().flatten()
            .or_else(|| inputs.get(GNSS_INDEX).copied().flatten());

        self.value = match raw {
            Some(raw) => {
                let heading_deg = raw as f32 / GNSS_HEADING_SCALE;
                self.last_heading_deg = Some(heading_deg);
                SensorValue::analog_with_constraints_and_metadata(
                    heading_deg.clamp(self.constraints.min_value, self.constraints.max_value),
                    self.constraints.clone(),
                    self.metadata.clone(),
                )
            }
            None => { 
                if let Some(last_heading_deg) = self.last_heading_deg {
                    SensorValue::analog_with_constraints_and_metadata(
                        last_heading_deg.clamp(self.constraints.min_value, self.constraints.max_value),
                        self.constraints.clone(),
                        self.metadata.clone(),
                    )
                } else {
                    SensorValue::new(ValueData::Empty, self.constraints.clone(), self.metadata.clone())
                }
            }
        };
        Ok(&self.value)
    }
}

/// Builds the heading fusion chain with providers in the fixed order HeadingFusionSensor::read
/// expects (BNO085 first, GNSS second -- see BNO085_INDEX/GNSS_INDEX above), so callers can't
/// get the order wrong by constructing SensorFusedAnalogInputChain directly.
pub fn new_heading_fusion_chain(bno_frame: Bno085Frame, gnss_frame: GnssFrame) -> SensorFusedAnalogInputChain {
    SensorFusedAnalogInputChain::new(
        HWInput::HwHeading,
        vec![
            Box::new(Bno085ChannelProvider::new(HWInput::HwBno085Heading, bno_frame)),
            Box::new(GnssChannelProvider::new(HWInput::HwGnssHeading, gnss_frame)),
        ],
        Box::new(HeadingFusionSensor::new()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(heading_deg: f32) -> u16 {
        (heading_deg * GNSS_HEADING_SCALE).round() as u16
    }

    #[test]
    fn test_prefers_bno085_when_both_available() {
        let mut sensor = HeadingFusionSensor::new();
        let value = sensor.read(&[Some(encode(90.0)), Some(encode(180.0))]).unwrap();
        assert!((value.as_f32() - 90.0).abs() < 0.01, "expected BNO085's 90°, got {}", value.as_f32());
    }

    #[test]
    fn test_falls_back_to_gnss_when_bno085_unavailable() {
        let mut sensor = HeadingFusionSensor::new();
        let value = sensor.read(&[None, Some(encode(180.0))]).unwrap();
        assert!((value.as_f32() - 180.0).abs() < 0.01, "expected GNSS's 180°, got {}", value.as_f32());
    }

    #[test]
    fn test_empty_when_neither_source_available() {
        let mut sensor = HeadingFusionSensor::new();
        let value = sensor.read(&[None, None]).unwrap();
        assert_eq!(value.value, ValueData::Empty);
    }

    #[test]
    fn test_stays_fresh_across_reads_when_source_flips() {
        // A BNO085 read that recovers after a stale period should immediately take over
        // again from GNSS on the very next read -- no latching/hysteresis.
        let mut sensor = HeadingFusionSensor::new();
        sensor.read(&[None, Some(encode(45.0))]).unwrap();
        let value = sensor.read(&[Some(encode(10.0)), Some(encode(45.0))]).unwrap();
        assert!((value.as_f32() - 10.0).abs() < 0.01, "expected BNO085 to immediately retake priority, got {}", value.as_f32());
    }
}
