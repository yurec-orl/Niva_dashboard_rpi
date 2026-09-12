use crate::graphics::context::GraphicsContext;
use crate::graphics::ui_style::*;
use crate::page_framework::page_manager::{Page, PageBase, PageButton, ButtonPosition, DIAG_PAGE_ID};
use crate::page_framework::events::{UIEvent, EventReceiver, SmartEventSender};
use crate::hardware::sensor_manager::SensorManager;
use crate::util::adc_data_provider::{OscFrame, OscCaptureState, OSC_BUF_LEN, OSC_SAMPLE_RATE_HZ};
use std::cell::Cell;
use std::rc::Rc;
use gl;

const TITLE_Y: f32 = 15.0;
const STATUS_Y: f32 = 45.0;

const GRAPH_TOP_MARGIN: f32 = 80.0;
const GRAPH_BOTTOM_MARGIN: f32 = 70.0;
// Leaves room for the physical button labels (rendered by PageManager, not this page) and,
// on the left, the amplitude-axis value labels.
const GRAPH_LEFT_MARGIN: f32 = 110.0;
const GRAPH_RIGHT_MARGIN: f32 = 110.0;

const TIME_GRID_STEP_MS: f32 = 10.0;
const AMPLITUDE_DIVISIONS: u32 = 4;
const WAVEFORM_THICKNESS: f32 = 2.0;

/// Time-axis view controls. The capture buffer spans OSC_TOTAL_MS (~81.9 ms =
/// OSC_BUF_LEN / OSC_SAMPLE_RATE_HZ). ЛУПА+/ЛУПА- walk the visible window through the
/// fixed OSC_ZOOM_WINDOWS_MS ladder -- 20 ms steps down to 20 ms, then 5 ms steps to a
/// 5 ms window for supply-ripple detail. <СДВИГ/СДВИГ> pan by one window width, capped at
/// OSC_PAN_STEP_MAX_MS. The widest rung is the nominal 80 ms rather than the true ~81.9 ms,
/// so at full zoom the ~2 ms tail can still be panned into view.
const OSC_TOTAL_MS: f32 = OSC_BUF_LEN as f32 / OSC_SAMPLE_RATE_HZ as f32 * 1000.0;
const OSC_ZOOM_WINDOWS_MS: [f32; 8] = [80.0, 60.0, 40.0, 20.0, 10.0, 5.0, 2.0, 1.0];
const OSC_PAN_STEP_MAX_MS: f32 = 20.0;

#[derive(Clone, Copy)]
struct OscView {
    zoom_idx: usize,
    offset_ms: f32,
}

impl OscView {
    fn full() -> Self {
        OscView { zoom_idx: 0, offset_ms: 0.0 }
    }

    fn window_ms(&self) -> f32 {
        OSC_ZOOM_WINDOWS_MS[self.zoom_idx]
    }

    fn max_offset(&self) -> f32 {
        (OSC_TOTAL_MS - self.window_ms()).max(0.0)
    }

    /// One window width per press, so the finer zoom rungs don't skip past regions --
    /// but never more than OSC_PAN_STEP_MAX_MS on the wide rungs.
    fn pan_step(&self) -> f32 {
        self.window_ms().min(OSC_PAN_STEP_MAX_MS)
    }

    fn zoom_in(&mut self) {
        if self.zoom_idx + 1 < OSC_ZOOM_WINDOWS_MS.len() {
            self.zoom_idx += 1;
        }
        self.offset_ms = self.offset_ms.min(self.max_offset());
    }

    fn zoom_out(&mut self) {
        self.zoom_idx = self.zoom_idx.saturating_sub(1);
        self.offset_ms = self.offset_ms.min(self.max_offset());
    }

    fn pan_left(&mut self) {
        self.offset_ms = (self.offset_ms - self.pan_step()).max(0.0);
    }

    fn pan_right(&mut self) {
        self.offset_ms = (self.offset_ms + self.pan_step()).min(self.max_offset());
    }
}

/// STM32 ADC1 is 12-bit, referenced to VDDA (3.3V) -- see OSCILLOSCOPE_DESIGN.md and
/// test/run_test.rs's dump_osc_buffer_csv, which uses the same conversion.
const OSC_ADC_MAX_CODE: f32 = 4095.0;
const OSC_ADC_VREF: f32 = 3.3;

/// PA3's voltage divider steps the car's 12V system voltage down to the ADC's 0-3.3V range
/// (see stm32_adc_module/WIRING.md's "PA3 -- 12V system voltage" section): R1=51kΩ from the
/// 12V line, R2=10kΩ to GND, ADC pin taps the R1/R2 junction. Real system voltage = ADC pin
/// voltage / (R2/(R1+R2)) -- i.e. multiplied back up by (R1+R2)/R2 (~6.1x).
const OSC_DIVIDER_R1_OHM: f32 = 51_000.0;
const OSC_DIVIDER_R2_OHM: f32 = 10_000.0;

/// Multiplicative calibration for the combined divider-resistor / ADC-reference / ADC-gain
/// error on the PA3 channel. Must equal `sensor_config.json`'s `Hw12v` `trim` -- same
/// physical circuit, converted here instead of through VoltageDividerSensor. A guard test
/// (osc_v12_trim_matches_sensor_config) fails if the two drift apart; removing the
/// duplication entirely is tracked as GitHub issue #12.
const OSC_V12_TRIM: f32 = 1.036;

/// Converts a raw ADC code to real 12V-system volts: ADC pin voltage scaled back up through
/// the PA3 divider (see OSC_DIVIDER_* doc comment above) and the OSC_V12_TRIM calibration.
fn adc_code_to_volts(code: f32) -> f32 {
    let adc_pin_volts = code * OSC_ADC_VREF / OSC_ADC_MAX_CODE;
    adc_pin_volts * (OSC_DIVIDER_R1_OHM + OSC_DIVIDER_R2_OHM) / OSC_DIVIDER_R2_OHM * OSC_V12_TRIM
}

const GRID_COLOR: (f32, f32, f32) = (0.25, 0.25, 0.25);
const AXIS_COLOR: (f32, f32, f32) = (0.6, 0.6, 0.6);
const WAVEFORM_COLOR: (f32, f32, f32) = (0.2, 1.0, 0.4);

/// Oscilloscope page: on entry (and on ЗАХВ), requests a burst capture from the STM32 ADC
/// module via OscFrame and renders the result as a time-amplitude graph. See
/// OSCILLOSCOPE_DESIGN.md for the capture protocol.
pub struct OscPage {
    base: PageBase,
    smart_event_sender: SmartEventSender,
    event_receiver: EventReceiver,

    // None when the ADC data provider never started -- there is nothing to capture from.
    osc_frame: Option<OscFrame>,

    // Raw samples from the most recently completed capture, if any.
    last_capture: Option<Vec<u16>>,
    // Shown above the graph while a capture is in flight or failed; cleared on success.
    status_message: Option<String>,

    // Time-axis zoom/pan state, shared with the ЛУПА/СДВИГ button callbacks.
    view: Rc<Cell<OscView>>,
}

impl OscPage {
    pub fn new(id: u32, smart_event_sender: SmartEventSender, event_receiver: EventReceiver, osc_frame: Option<OscFrame>) -> Self {
        let mut page = OscPage {
            base: PageBase::new(id, "Osc".to_string()),
            smart_event_sender,
            event_receiver,
            osc_frame,
            last_capture: None,
            status_message: None,
            view: Rc::new(Cell::new(OscView::full())),
        };
        page.setup_buttons();
        page
    }

    fn setup_buttons(&mut self) {
        let buttons = vec![
            PageButton::new(ButtonPosition::Left1, "ЛУПА+".into(), Box::new({
                let view = self.view.clone();
                move || { let mut v = view.get(); v.zoom_in(); view.set(v); }
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left2, "ЛУПА-".into(), Box::new({
                let view = self.view.clone();
                move || { let mut v = view.get(); v.zoom_out(); view.set(v); }
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Left4, "ЗАХВ".into(), Box::new({
                let osc_frame = self.osc_frame.clone();
                move || {
                    if let Some(osc) = &osc_frame {
                        osc.request_capture();
                    }
                }
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right1, "<СДВИГ".into(), Box::new({
                let view = self.view.clone();
                move || { let mut v = view.get(); v.pan_left(); view.set(v); }
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right2, "СДВИГ>".into(), Box::new({
                let view = self.view.clone();
                move || { let mut v = view.get(); v.pan_right(); view.set(v); }
            }) as Box<dyn FnMut()>),
            PageButton::new(ButtonPosition::Right4, "ВОЗВР".into(), Box::new({
                let sender = self.smart_event_sender.clone();
                move || sender.send(UIEvent::SwitchToPage(DIAG_PAGE_ID))
            }) as Box<dyn FnMut()>),
        ];
        self.base.set_buttons(buttons);
    }

    fn request_capture(&mut self) {
        match &self.osc_frame {
            Some(osc) => {
                osc.request_capture();
                self.status_message = Some("ЗАХВАТ...".to_string());
            }
            None => self.status_message = Some("АЦП НЕДОСТУПНО".to_string()),
        }
    }
}

impl Page for OscPage {
    fn id(&self) -> u32 {
        self.base.id()
    }

    fn name(&self) -> &str {
        self.base.name()
    }

    fn set_buttons(&mut self, buttons: Vec<PageButton<Box<dyn FnMut()>>>) {
        self.base.set_buttons(buttons);
    }

    fn render(&self, context: &mut GraphicsContext, _sensor_manager: &SensorManager, ui_style: &UIStyle) -> Result<(), String> {
        let title_font = ui_style.get_string(StyleKey::TextPrimaryFont);
        let title_font_size = ui_style.get_integer(StyleKey::TextPrimaryFontSize);
        let title_color = ui_style.get_color(StyleKey::TerminalTextColor);
        let text_font = ui_style.get_string(StyleKey::TextMonospaceFont);
        let text_font_size = ui_style.get_integer(StyleKey::TextMonospaceFontSize);
        let text_color = ui_style.get_color(StyleKey::TerminalTextColor);

        context.render_text_with_font(
            "ОСЦИЛЛОГРАФ (БОРТ СЕТЬ)", GRAPH_LEFT_MARGIN, TITLE_Y, 1.0, title_color, &title_font, title_font_size,
        )?;

        if let Some(status) = &self.status_message {
            context.render_text_with_font(status, GRAPH_LEFT_MARGIN, STATUS_Y, 1.0, text_color, &text_font, text_font_size)?;
        }

        let view = self.view.get();
        let window_ms = view.window_ms();
        let view_label = format!("ОКНО {:.0}мс  СДВИГ {:.0}мс", window_ms, view.offset_ms);
        let view_label_w = context.calculate_text_width_with_font(&view_label, 1.0, &text_font, text_font_size)?;
        context.render_text_with_font(
            &view_label, context.width as f32 - GRAPH_RIGHT_MARGIN - view_label_w, TITLE_Y + 2.0,
            1.0, text_color, &text_font, text_font_size,
        )?;

        let samples = match &self.last_capture {
            Some(s) if s.len() > 1 => s,
            _ => return Ok(()),
        };

        let screen_w = context.width as f32;
        let screen_h = context.height as f32;
        let graph_x0 = GRAPH_LEFT_MARGIN;
        let graph_x1 = screen_w - GRAPH_RIGHT_MARGIN;
        let graph_y0 = GRAPH_TOP_MARGIN;
        let graph_y1 = screen_h - GRAPH_BOTTOM_MARGIN;
        let graph_w = graph_x1 - graph_x0;
        let graph_h = graph_y1 - graph_y0;

        // Visible time window and the sample index range covering it -- one extra sample each
        // side so the drawn trace stays continuous where it leaves the grid. Shared by the
        // stats/auto-scale below and the waveform draw further down.
        let t_start = view.offset_ms;
        let t_end = view.offset_ms + window_ms;
        let n = samples.len();
        let sample_ms = OSC_TOTAL_MS / (n - 1) as f32;
        let first = ((t_start / sample_ms).floor() as isize - 1).max(0) as usize;
        let last = (((t_end / sample_ms).ceil() as usize) + 1).min(n);

        // MIN/MAX/Δ and the amplitude auto-scale cover only the visible slice, so they
        // describe what's currently on screen rather than the whole ~82 ms capture. Scaling
        // to the slice's own min/max (not the full 12-bit ADC range) also makes the waveform
        // fill the available vertical space at any zoom. A little padding keeps peaks off the
        // grid edge; a flat slice (min == max) falls back to a 1-unit span so the math below
        // stays well-defined.
        let visible = &samples[first..last];
        let data_min = *visible.iter().min().unwrap() as f32;
        let data_max = *visible.iter().max().unwrap() as f32;

        let min_v = adc_code_to_volts(data_min);
        let max_v = adc_code_to_volts(data_max);
        let stats_label = format!("MIN {:.2}В  MAX {:.2}В  Δ {:.2}В", min_v, max_v, max_v - min_v);
        let stats_label_w = context.calculate_text_width_with_font(&stats_label, 1.0, &text_font, text_font_size)?;
        context.render_text_with_font(
            &stats_label, graph_x1 - stats_label_w, STATUS_Y, 1.0, text_color, &text_font, text_font_size,
        )?;

        let span = (data_max - data_min).max(1.0);
        let pad = span * 0.05;
        let y_min = data_min - pad;
        let y_max = data_max + pad;
        let y_span = (y_max - y_min).max(1.0);

        // Amplitude grid lines + value labels, in real 12V-system volts (ADC pin voltage
        // scaled back up through the PA3 divider -- see OSC_DIVIDER_* doc comment above).
        // The graph itself still scales in raw ADC codes -- only the label text is converted
        // -- since the samples are raw codes throughout.
        for i in 0..=AMPLITUDE_DIVISIONS {
            let frac = i as f32 / AMPLITUDE_DIVISIONS as f32;
            let y = graph_y1 - frac * graph_h;
            let value = y_min + frac * y_span;
            context.render_line((graph_x0, y), (graph_x1, y), GRID_COLOR, 1.0)?;
            let label = format!("{:.2}В", adc_code_to_volts(value));
            let label_w = context.calculate_text_width_with_font(&label, 1.0, &text_font, text_font_size)?;
            context.render_text_with_font(&label, graph_x0 - label_w - 8.0, y - text_font_size as f32 * 0.5, 1.0, text_color, &text_font, text_font_size)?;
        }

        // Time grid lines + labels, every TIME_GRID_STEP_MS across the visible window
        // [t_start, t_end]; labels are absolute capture time.
        let mut t = (t_start / TIME_GRID_STEP_MS).ceil() * TIME_GRID_STEP_MS;
        while t <= t_end + 0.01 {
            let x = graph_x0 + (t - t_start) / window_ms * graph_w;
            context.render_line((x, graph_y0), (x, graph_y1), GRID_COLOR, 1.0)?;
            let label = format!("{:.0}мс", t);
            context.render_text_with_font(&label, x - 12.0, graph_y1 + 8.0, 1.0, text_color, &text_font, text_font_size)?;
            t += TIME_GRID_STEP_MS;
        }

        // Axis lines, brighter than the grid.
        context.render_line((graph_x0, graph_y0), (graph_x0, graph_y1), AXIS_COLOR, 1.5)?;
        context.render_line((graph_x0, graph_y1), (graph_x1, graph_y1), AXIS_COLOR, 1.5)?;

        // Signal waveform -- the visible-window sample slice computed above, mapped so that
        // window fills the graph width, in one batched draw call (see osc_waveform module doc).
        let points: Vec<(f32, f32)> = (first..last).map(|i| {
            let t_ms = i as f32 * sample_ms;
            let x = graph_x0 + (t_ms - t_start) / window_ms * graph_w;
            let y = graph_y1 - ((samples[i] as f32 - y_min) / y_span) * graph_h;
            (x, y)
        }).collect();

        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            let color = context.apply_brightness(WAVEFORM_COLOR);
            osc_waveform::draw_polyline(&points, WAVEFORM_THICKNESS, color, screen_w, screen_h);
        }

        Ok(())
    }

    fn on_enter(&mut self) -> Result<(), String> {
        log::info!("Entering Oscilloscope page");
        self.view.set(OscView::full());
        self.request_capture();
        Ok(())
    }

    fn on_exit(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn on_button(&mut self, _button: char) -> Result<(), String> {
        Ok(())
    }

    fn process_events(&mut self) {
        while self.event_receiver.try_recv().is_ok() {}

        if let Some(osc) = &self.osc_frame {
            match osc.state() {
                OscCaptureState::Idle => {}
                OscCaptureState::Capturing => {
                    self.status_message = Some("ЗАХВАТ...".to_string());
                }
                OscCaptureState::Done(samples) => {
                    self.last_capture = Some(samples);
                    self.status_message = None;
                }
                OscCaptureState::Failed(err) => {
                    self.status_message = Some(format!("ОШИБКА ЗАХВАТА: {}", err));
                }
            }
        }
    }

    fn buttons(&self) -> &Vec<PageButton<Box<dyn FnMut()>>> {
        self.base.buttons()
    }

    fn button_by_position(&self, pos: ButtonPosition) -> Option<&PageButton<Box<dyn FnMut()>>> {
        self.base.button_by_position(pos)
    }

    fn button_by_position_mut(&mut self, pos: ButtonPosition) -> Option<&mut PageButton<Box<dyn FnMut()>>> {
        self.base.button_by_position_mut(pos)
    }
}

/// Batched thick-polyline rendering for the waveform trace. Kept separate from
/// GraphicsContext::render_line, which issues one draw call per segment -- fine for the
/// handful of grid/axis lines above, but a single capture has up to OSC_BUF_LEN-1 (4095)
/// segments, and driving that many discrete draw calls per frame would tank the frame rate.
/// Instead all segments are batched into one vertex buffer and drawn with a single
/// glDrawArrays call, following the same persistent-VBO pattern as
/// indicators::needle_indicator's NeedleGaugeMarksDecorator (see CLAUDE.md's render-loop
/// performance rule: never glGenBuffers/glDeleteBuffers inside a per-frame render path).
mod osc_waveform {
    use std::sync::Once;
    use gl;

    static SHADER_INIT: Once = Once::new();
    static mut SHADER_PROGRAM: u32 = 0;
    static VBO_INIT: Once = Once::new();
    static mut VBO: u32 = 0;

    unsafe fn get_shader() -> u32 {
        SHADER_INIT.call_once(|| {
            let vertex_shader_source = b"
attribute vec2 position;
attribute vec3 color;
varying vec3 v_color;
void main() {
    gl_Position = vec4(position, 0.0, 1.0);
    v_color = color;
}
\0";
            let fragment_shader_source = b"
precision mediump float;
varying vec3 v_color;
void main() {
    gl_FragColor = vec4(v_color, 1.0);
}
\0";
            let vertex_shader = gl::CreateShader(gl::VERTEX_SHADER);
            gl::ShaderSource(vertex_shader, 1, &vertex_shader_source.as_ptr(), std::ptr::null());
            gl::CompileShader(vertex_shader);

            let fragment_shader = gl::CreateShader(gl::FRAGMENT_SHADER);
            gl::ShaderSource(fragment_shader, 1, &fragment_shader_source.as_ptr(), std::ptr::null());
            gl::CompileShader(fragment_shader);

            let program = gl::CreateProgram();
            gl::AttachShader(program, vertex_shader);
            gl::AttachShader(program, fragment_shader);
            gl::LinkProgram(program);

            gl::DeleteShader(vertex_shader);
            gl::DeleteShader(fragment_shader);

            SHADER_PROGRAM = program;
        });
        SHADER_PROGRAM
    }

    unsafe fn get_vbo() -> u32 {
        VBO_INIT.call_once(|| {
            gl::GenBuffers(1, &raw mut VBO);
        });
        VBO
    }

    /// Draws a `thickness`-px polyline through `points` (screen-space pixel coordinates) as
    /// one batched draw call: each segment becomes a quad (2 triangles), all quads land in a
    /// single vertex buffer converted to clip space up front.
    pub unsafe fn draw_polyline(points: &[(f32, f32)], thickness: f32, color: (f32, f32, f32), screen_w: f32, screen_h: f32) {
        if points.len() < 2 {
            return;
        }

        let half = thickness / 2.0;
        let mut vertices: Vec<f32> = Vec::with_capacity((points.len() - 1) * 30);
        for pair in points.windows(2) {
            let (x0, y0) = pair[0];
            let (x1, y1) = pair[1];
            let dx = x1 - x0;
            let dy = y1 - y0;
            let len = (dx * dx + dy * dy).sqrt();
            let (nx, ny) = if len > 0.0 { (-dy / len * half, dx / len * half) } else { (half, 0.0) };

            let corners = [
                (x0 + nx, y0 + ny), (x1 + nx, y1 + ny), (x0 - nx, y0 - ny),
                (x1 + nx, y1 + ny), (x1 - nx, y1 - ny), (x0 - nx, y0 - ny),
            ];
            for (x, y) in corners {
                vertices.push(x / screen_w * 2.0 - 1.0);
                vertices.push(1.0 - y / screen_h * 2.0);
                vertices.push(color.0);
                vertices.push(color.1);
                vertices.push(color.2);
            }
        }

        let shader_program = get_shader();
        gl::UseProgram(shader_program);

        let vbo = get_vbo();
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (vertices.len() * std::mem::size_of::<f32>()) as isize,
            vertices.as_ptr() as *const std::ffi::c_void,
            gl::DYNAMIC_DRAW,
        );

        let pos_attr = gl::GetAttribLocation(shader_program, b"position\0".as_ptr()) as u32;
        let color_attr = gl::GetAttribLocation(shader_program, b"color\0".as_ptr()) as u32;
        gl::EnableVertexAttribArray(pos_attr);
        gl::VertexAttribPointer(pos_attr, 2, gl::FLOAT, gl::FALSE, 20, std::ptr::null());
        gl::EnableVertexAttribArray(color_attr);
        gl::VertexAttribPointer(color_attr, 3, gl::FLOAT, gl::FALSE, 20, 8 as *const _);

        let vertex_count = (vertices.len() / 5) as i32;
        gl::DrawArrays(gl::TRIANGLES, 0, vertex_count);
    }
}

#[cfg(test)]
mod tests {
    use super::OSC_V12_TRIM;
    use std::path::Path;

    /// OSC_V12_TRIM duplicates `sensor_config.json`'s `Hw12v` `trim` (see the constant's doc
    /// comment). This fails `cargo test` the moment the two drift, which is the only thing
    /// keeping the duplication safe until the shared-conversion refactor lands (issue #12).
    #[test]
    fn osc_v12_trim_matches_sensor_config() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("sensor_config.json");
        let json: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("repo sensor_config.json should be readable"),
        )
        .expect("sensor_config.json should be valid JSON");

        let hw12v = json
            .as_array()
            .expect("sensor_config.json is a JSON array")
            .iter()
            .find(|entry| {
                entry.pointer("/sensor/id").and_then(|v| v.as_str()) == Some("Hw12v")
            })
            .expect("an Hw12v sensor entry in sensor_config.json");
        let trim = hw12v
            .pointer("/sensor/trim")
            .expect("the Hw12v entry has a `trim` field")
            .as_f64()
            .expect("`trim` is a number") as f32;

        assert!(
            (trim - OSC_V12_TRIM).abs() < 1e-6,
            "sensor_config.json Hw12v trim ({trim}) != osc_page OSC_V12_TRIM ({OSC_V12_TRIM}) -- \
             update the constant in osc_page.rs to match",
        );
    }
}
